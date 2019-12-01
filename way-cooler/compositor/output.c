#define _POSIX_C_SOURCE 200809L

#include "compositor/output.h"

#include <assert.h>
#include <stdlib.h>
#include <time.h>

#include <wayland-server.h>
#include <wlr/render/wlr_renderer.h>
#include <wlr/types/wlr_matrix.h>
#include <wlr/types/wlr_output.h>
#include <wlr/types/wlr_output_damage.h>
#include <wlr/types/wlr_output_layout.h>
#include <wlr/util/log.h>
#include <wlr/util/region.h>

#include "compositor/layer_shell.h"
#include "compositor/server.h"
#include "compositor/view.h"

struct wc_render_data {
	struct wlr_renderer *renderer;
	pixman_region32_t *damage;
	struct timespec *when;
};

/* Used to move all of the data necessary to render a surface from the top-level
 * frame handler to the per-surface render function. */
struct wc_view_render_data {
	struct wlr_output *output;
	struct wc_view *view;
	struct wc_render_data render_data;
};

/* Used to move all of the data necessary to render a surface from the layers */
struct wc_layer_render_data {
	struct wc_layer *layer;
	struct wc_render_data render_data;
};

/* Used when calculating the damage of a surface */
struct wc_surface_damage_data {
	struct wc_output *output;
	pixman_region32_t *surface_damage;
	// The full size of the surface, used if the surface_damage is NULL
	struct wlr_box surface_output_geo;
};

static void damage_surface_iterator(
		struct wlr_surface *surface, int sx, int sy, void *data_) {
	struct wc_surface_damage_data *damage_data = data_;

	struct wlr_box surface_area = damage_data->surface_output_geo;
	surface_area.x += sx;
	surface_area.y += sy;

	struct wc_output *output = damage_data->output;
	if (damage_data->surface_damage == NULL) {
		wlr_output_damage_add_box(output->damage, &surface_area);
	} else {
		wlr_output_damage_add(output->damage, damage_data->surface_damage);
	}
	wlr_output_schedule_frame(output->wlr_output);
}

static void scissor_output(
		struct wlr_output *wlr_output, pixman_box32_t *rect) {
	struct wlr_renderer *renderer =
			wlr_backend_get_renderer(wlr_output->backend);
	assert(renderer);

	struct wlr_box box = {
			.x = rect->x1,
			.y = rect->y1,
			.width = rect->x2 - rect->x1,
			.height = rect->y2 - rect->y1,
	};

	int ow, oh;
	wlr_output_transformed_resolution(wlr_output, &ow, &oh);

	enum wl_output_transform transform =
			wlr_output_transform_invert(wlr_output->transform);
	wlr_box_transform(&box, &box, transform, ow, oh);

	wlr_renderer_scissor(renderer, &box);
}

static void wc_render_surface(struct wlr_surface *surface,
		pixman_region32_t *damage, struct wlr_output *output,
		struct wlr_renderer *renderer, struct timespec *when, int sx, int sy,
		int ox, int oy) {
	struct wlr_texture *texture = wlr_surface_get_texture(surface);
	if (texture == NULL) {
		return;
	}

	struct wlr_box box = {
			.x = sx + ox,
			.y = sy + oy,
			.width = surface->current.width * output->scale,
			.height = surface->current.height * output->scale,
	};
	float matrix[9];
	enum wl_output_transform transform =
			wlr_output_transform_invert(surface->current.transform);
	wlr_matrix_project_box(
			matrix, &box, transform, 0, output->transform_matrix);

	int nrects;
	pixman_box32_t *rects = pixman_region32_rectangles(damage, &nrects);
	for (int i = 0; i < nrects; i++) {
		scissor_output(output, &rects[i]);
		wlr_render_texture_with_matrix(renderer, texture, matrix, 1);
	}

	wlr_surface_send_frame_done(surface, when);
}

static void wc_render_view(
		struct wlr_surface *surface, int sx, int sy, void *data) {
	struct wc_view_render_data *rdata = data;
	pixman_region32_t *damage = rdata->render_data.damage;
	struct wc_view *view = rdata->view;
	struct wlr_output *output = rdata->output;

	double ox = 0, oy = 0;
	wlr_output_layout_output_coords(
			view->server->output_layout, output, &ox, &oy);
	ox += view->geo.x + sx;
	oy += view->geo.y + sy;

	wc_render_surface(surface, damage, output, rdata->render_data.renderer,
			rdata->render_data.when, sx, sy, ox, oy);
}

static void wc_render_layer(
		struct wlr_surface *surface, int sx, int sy, void *data) {
	struct wc_layer_render_data *rdata = data;
	pixman_region32_t *damage = rdata->render_data.damage;
	struct wc_layer *layer = rdata->layer;
	struct wc_server *server = layer->server;
	struct wlr_output *output = layer->layer_surface->output;

	double ox = 0, oy = 0;
	wlr_output_layout_output_coords(server->output_layout, output, &ox, &oy);
	ox += layer->geo.x + sx, oy += layer->geo.y + sy;

	wc_render_surface(surface, damage, output, rdata->render_data.renderer,
			rdata->render_data.when, sx, sy, layer->geo.x, layer->geo.y);
}

static void wc_render_layers(struct timespec *now, pixman_region32_t *damage,
		struct wlr_renderer *renderer, struct wc_output *output,
		struct wl_list *layers) {
	struct wc_layer *layer;
	wl_list_for_each_reverse(layer, layers, link) {
		if (!layer->mapped) {
			continue;
		}
		struct wc_render_data render_data = {
				.renderer = renderer,
				.damage = damage,
				.when = now,
		};
		struct wc_layer_render_data rdata = {
				.layer = layer,
				.render_data = render_data,
		};

		wlr_layer_surface_v1_for_each_surface(
				layer->layer_surface, wc_render_layer, &rdata);
	}
}

static void wc_output_frame(struct wl_listener *listener, void *data) {
	struct wc_output *output = wl_container_of(listener, output, frame);
	struct wc_server *server = output->server;
	struct wlr_output *wlr_output = output->wlr_output;
	struct wlr_renderer *renderer =
			wlr_backend_get_renderer(wlr_output->backend);
	assert(renderer);

	struct timespec now;
	clock_gettime(CLOCK_MONOTONIC, &now);

	bool needs_swap = false;
	pixman_region32_t damage;
	pixman_region32_init(&damage);
	if (!wlr_output_damage_attach_render(
				output->damage, &needs_swap, &damage)) {
		return;
	}

	if (!needs_swap) {
		goto damage_finish;
	}

	wlr_renderer_begin(renderer, wlr_output->width, wlr_output->height);

	if (!pixman_region32_not_empty(&damage)) {
		goto renderer_end;
	}

	if (WC_DEBUG) {
		wlr_renderer_clear(renderer, (float[]){1, 1, 0, 1});
	}

	float background_color[4] = {0.0f, 0.0f, 0.0f, 1};
	int nrects;
	pixman_box32_t *rects = pixman_region32_rectangles(&damage, &nrects);
	for (int i = 0; i < nrects; i++) {
		scissor_output(output->wlr_output, &rects[i]);
		wlr_renderer_clear(renderer, background_color);
	}

	struct wl_list *backgrounds =
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_BACKGROUND];
	struct wl_list *bottom = &output->layers[ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM];
	struct wl_list *top = &output->layers[ZWLR_LAYER_SHELL_V1_LAYER_TOP];
	struct wl_list *overlay =
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY];

	wc_render_layers(&now, &damage, renderer, output, backgrounds);
	wc_render_layers(&now, &damage, renderer, output, bottom);

	// Render traditional shell surfaces between bottom and top layers.
	struct wc_view *view;
	wl_list_for_each_reverse(view, &server->views, link) {
		if (!view->mapped) {
			continue;
		}
		struct wc_render_data render_data = {
				.renderer = renderer,
				.damage = &damage,
				.when = &now,
		};
		struct wc_view_render_data rdata = {
				.output = output->wlr_output,
				.view = view,
				.render_data = render_data,
		};

		wc_view_for_each_surface(view, wc_render_view, &rdata);
	}

	wc_render_layers(&now, &damage, renderer, output, top);
	wc_render_layers(&now, &damage, renderer, output, overlay);

renderer_end:
	wlr_output_render_software_cursors(wlr_output, &damage);
	wlr_renderer_scissor(renderer, NULL);
	wlr_renderer_end(renderer);

	int width, height;
	wlr_output_transformed_resolution(wlr_output, &width, &height);

	if (WC_DEBUG) {
		pixman_region32_union_rect(&damage, &damage, 0, 0, width, height);
	}

	enum wl_output_transform transform =
			wlr_output_transform_invert(wlr_output->transform);
	wlr_region_transform(&damage, &damage, transform, width, height);
	wlr_output_set_damage(wlr_output, &damage);
	wlr_output_commit(wlr_output);

damage_finish:
	pixman_region32_fini(&damage);
}

static void wc_output_destroy(struct wl_listener *listener, void *data) {
	struct wc_output *output = wl_container_of(listener, output, destroy);
	struct wc_server *server = output->server;
	wl_list_remove(&output->link);

	wl_list_remove(&output->frame.link);
	wl_list_remove(&output->destroy.link);

	if (server->active_output == output) {
		server->active_output = NULL;
		if (!wl_list_empty(&server->outputs)) {
			server->active_output = wl_container_of(
					server->outputs.prev, server->active_output, link);
		}
	}

	size_t len = sizeof(output->layers) / sizeof(output->layers[0]);
	for (size_t i = 0; i < len; i++) {
		struct wc_layer *layer;
		struct wc_layer *temp;
		wl_list_for_each_safe(layer, temp, &output->layers[i], link) {
			wc_layer_shell_destroy(&layer->destroy, NULL);
		}
	}

	if (server->output_layout != NULL) {
		wlr_output_layout_remove(server->output_layout, output->wlr_output);
	}
	wlr_output_destroy_global(output->wlr_output);

	free(output);
}

static void wc_new_output(struct wl_listener *listener, void *data) {
	struct wc_server *server = wl_container_of(listener, server, new_output);
	struct wlr_output *wlr_output = data;

	if (!wl_list_empty(&wlr_output->modes)) {
		struct wlr_output_mode *mode =
				wl_container_of(wlr_output->modes.prev, mode, link);
		wlr_output_set_mode(wlr_output, mode);
	}

	struct wc_output *output = calloc(1, sizeof(struct wc_output));
	output->wlr_output = wlr_output;
	output->server = server;
	wlr_output->data = output;
	output->damage = wlr_output_damage_create(wlr_output);

	size_t len = sizeof(output->layers) / sizeof(output->layers[0]);
	for (size_t i = 0; i < len; i++) {
		wl_list_init(&output->layers[i]);
	}

	output->frame.notify = wc_output_frame;
	output->destroy.notify = wc_output_destroy;

	wl_signal_add(&output->damage->events.frame, &output->frame);
	wl_signal_add(&wlr_output->events.destroy, &output->destroy);

	wl_list_insert(&server->outputs, &output->link);

	if (server->active_output == NULL) {
		server->active_output = output;
	}

	wlr_output_layout_add_auto(server->output_layout, wlr_output);
	wlr_output_create_global(wlr_output);

	wc_layer_shell_arrange_layers(output);
	struct wc_output *temp;
	wl_list_for_each_safe(output, temp, &server->outputs, link) {
		wlr_output_damage_add_whole(output->damage);
	}
}

struct wc_output *wc_get_active_output(struct wc_server *server) {
	if (wl_list_empty(&server->outputs)) {
		return NULL;
	}
	struct wc_output *output = server->active_output;
	if (output == NULL) {
		output = wl_container_of(server->outputs.prev, output, link);
	}
	return output;
}

void wc_output_damage_surface(struct wc_output *output,
		struct wlr_surface *surface, pixman_region32_t *surface_damage,
		struct wlr_box surface_output_geo) {
	struct wc_surface_damage_data damage_data = {
			.output = output,
			.surface_output_geo = surface_output_geo,
			.surface_damage = surface_damage,
	};
	wlr_surface_for_each_surface(
			surface, damage_surface_iterator, &damage_data);
}

void wc_output_init(struct wc_server *server) {
	server->output_layout = wlr_output_layout_create();

	wl_list_init(&server->outputs);
	server->new_output.notify = wc_new_output;

	wl_signal_add(&server->backend->events.new_output, &server->new_output);
}

void wc_output_fini(struct wc_server *server) {
	struct wc_output *output;
	struct wc_output *temp;
	wl_list_for_each_safe(output, temp, &server->outputs, link) {
		wc_output_destroy(&output->destroy, NULL);
	}

	wlr_output_layout_destroy(server->output_layout);
	server->output_layout = NULL;

	wl_list_remove(&server->new_output.link);
}
