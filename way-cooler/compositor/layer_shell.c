#include "compositor/layer_shell.h"

#include <stdlib.h>

#include <wayland-server.h>
#include <wlr/types/wlr_box.h>
#include <wlr/types/wlr_layer_shell_v1.h>
#include <wlr/types/wlr_output.h>
#include <wlr/types/wlr_output_damage.h>
#include <wlr/util/log.h>

#include "compositor/output.h"
#include "compositor/seat.h"
#include "compositor/server.h"
#include "compositor/view.h"

static const uint32_t LAYER_BOTH_HORIZ =
		ZWLR_LAYER_SURFACE_V1_ANCHOR_LEFT | ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT;
static const uint32_t LAYER_BOTH_VERT =
		ZWLR_LAYER_SURFACE_V1_ANCHOR_TOP | ZWLR_LAYER_SURFACE_V1_ANCHOR_BOTTOM;

static void wc_layer_shell_commit(struct wl_listener *listener, void *data) {
	struct wc_layer *layer = wl_container_of(listener, layer, commit);
	struct wlr_layer_surface_v1 *layer_surface = layer->layer_surface;
	struct wlr_output *wlr_output = layer_surface->output;
	if (wlr_output == NULL) {
		return;
	}
	struct wc_output *wc_output = wlr_output->data;
	struct wlr_box old_geo = layer->geo;
	wc_layer_shell_arrange_layers(wlr_output->data);

	bool geo_changed =
			memcmp(&old_geo, &layer->geo, sizeof(struct wlr_box)) != 0;
	bool layer_changed = layer->layer != layer_surface->current.layer;
	if (layer_changed) {
		wl_list_remove(&layer->link);
		wl_list_insert(
				&wc_output->layers[layer_surface->current.layer], &layer->link);
		layer->layer = layer_surface->current.layer;
	}
	if (geo_changed || layer_changed) {
		wc_output_damage_surface(
				wlr_output->data, layer_surface->surface, NULL, old_geo);
	}

	pixman_region32_t damage;
	pixman_region32_init(&damage);
	wlr_surface_get_effective_damage(layer_surface->surface, &damage);
	pixman_region32_translate(&damage, layer->geo.x, layer->geo.y);
	wc_output_damage_surface(
			wlr_output->data, layer_surface->surface, &damage, layer->geo);

	pixman_region32_fini(&damage);
}

static void wc_layer_shell_map(struct wl_listener *listener, void *data) {
	struct wc_layer *layer = wl_container_of(listener, layer, map);
	struct wlr_layer_surface_v1 *layer_surface = layer->layer_surface;
	layer->mapped = true;

	pixman_region32_t damage;
	pixman_region32_init(&damage);
	wlr_surface_get_effective_damage(layer_surface->surface, &damage);
	pixman_region32_translate(&damage, layer->geo.x, layer->geo.y);

	wc_output_damage_surface(layer_surface->output->data,
			layer_surface->surface, &damage, layer->geo);

	pixman_region32_fini(&damage);
}

static void wc_layer_shell_unmap(struct wl_listener *listener, void *data) {
	struct wc_layer *layer = wl_container_of(listener, layer, unmap);
	struct wlr_layer_surface_v1 *layer_surface = layer->layer_surface;
	layer->mapped = false;

	wc_output_damage_surface(layer_surface->output->data,
			layer_surface->surface, NULL, layer->geo);
}

void wc_layer_shell_destroy(struct wl_listener *listener, void *data) {
	struct wc_layer *layer = wl_container_of(listener, layer, destroy);
	wl_list_remove(&layer->link);

	wl_list_remove(&layer->commit.link);
	wl_list_remove(&layer->map.link);
	wl_list_remove(&layer->unmap.link);
	wl_list_remove(&layer->destroy.link);

	wlr_layer_surface_v1_close(layer->layer_surface);
	free(layer);
}

static void wc_arrange_layer(struct wc_output *output, struct wc_seat *seat,
		struct wl_list *layers, struct wlr_box *usable_area, bool exclusive) {
	struct wlr_box full_area = {0};
	wlr_output_effective_resolution(
			output->wlr_output, &full_area.width, &full_area.height);
	struct wc_layer *wc_layer;
	wl_list_for_each_reverse(wc_layer, layers, link) {
		struct wlr_layer_surface_v1 *layer = wc_layer->layer_surface;
		struct wlr_layer_surface_v1_state *state = &layer->current;
		if (exclusive != (state->exclusive_zone > 0)) {
			continue;
		}
		struct wlr_box bounds = *usable_area;
		if (state->exclusive_zone == -1) {
			bounds = full_area;
		}
		struct wlr_box arranged_area = {
				.width = state->desired_width,
				.height = state->desired_height,
		};

		// horizontal axis
		if ((state->anchor & LAYER_BOTH_HORIZ) && arranged_area.width == 0) {
			arranged_area.x = bounds.x;
			arranged_area.width = bounds.width;
		} else if (state->anchor & ZWLR_LAYER_SURFACE_V1_ANCHOR_LEFT) {
			arranged_area.x = bounds.x;
		} else if (state->anchor & ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT) {
			arranged_area.x = bounds.x + (bounds.width - arranged_area.width);
		} else {
			arranged_area.x =
					bounds.x + ((bounds.width / 2) - (arranged_area.width / 2));
		}

		// vertical axis
		if ((state->anchor & LAYER_BOTH_VERT) && arranged_area.height == 0) {
			arranged_area.y = bounds.y;
			arranged_area.height = bounds.height;
		} else if (state->anchor & ZWLR_LAYER_SURFACE_V1_ANCHOR_TOP) {
			arranged_area.y = bounds.y;
		} else if (state->anchor & ZWLR_LAYER_SURFACE_V1_ANCHOR_BOTTOM) {
			arranged_area.y = bounds.y + (bounds.height - arranged_area.height);
		} else {
			arranged_area.y = bounds.y +
					((bounds.height / 2) - (arranged_area.height / 2));
		}

		// left and right margin
		if ((state->anchor & LAYER_BOTH_HORIZ) == LAYER_BOTH_HORIZ) {
			arranged_area.x += state->margin.left;
			arranged_area.width -= state->margin.left + state->margin.right;
		} else if (state->anchor & ZWLR_LAYER_SURFACE_V1_ANCHOR_LEFT) {
			arranged_area.x += state->margin.left;
		} else if (state->anchor & ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT) {
			arranged_area.x -= state->margin.right;
		}

		// top and bottom margin
		if ((state->anchor & LAYER_BOTH_VERT) == LAYER_BOTH_VERT) {
			arranged_area.y += state->margin.top;
			arranged_area.height -= state->margin.top + state->margin.bottom;
		} else if (state->anchor & ZWLR_LAYER_SURFACE_V1_ANCHOR_TOP) {
			arranged_area.y += state->margin.top;
		} else if (state->anchor & ZWLR_LAYER_SURFACE_V1_ANCHOR_BOTTOM) {
			arranged_area.y -= state->margin.bottom;
		}

		if (arranged_area.width < 0 || arranged_area.height < 0) {
			wlr_log(WLR_ERROR, "Bad width/height: %d, %d", arranged_area.width,
					arranged_area.height);
			wlr_layer_surface_v1_close(layer);
			continue;
		}

		wc_layer->geo = arranged_area;
		// TODO Apply exclusive zones
		wlr_layer_surface_v1_configure(
				layer, arranged_area.width, arranged_area.height);

		// TODO send cursor enter events if it's now hovering
	}
}

void wc_layer_shell_arrange_layers(struct wc_output *output) {
	struct wlr_box usable_area = {0};
	struct wc_server *server = output->server;
	struct wc_seat *seat = server->seat;
	wlr_output_effective_resolution(
			output->wlr_output, &usable_area.width, &usable_area.height);
	wc_arrange_layer(output, seat,
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY], &usable_area,
			true);
	wc_arrange_layer(output, seat,
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_TOP], &usable_area, true);
	wc_arrange_layer(output, seat,
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM], &usable_area,
			true);
	wc_arrange_layer(output, seat,
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_BACKGROUND], &usable_area,
			true);

	// TODO Arrange maximized views once we have those

	wc_arrange_layer(output, seat,
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY], &usable_area,
			false);
	wc_arrange_layer(output, seat,
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_TOP], &usable_area,
			false);
	wc_arrange_layer(output, seat,
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM], &usable_area,
			false);
	wc_arrange_layer(output, seat,
			&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_BACKGROUND], &usable_area,
			false);

	uint32_t layers_above_shell[] = {
			ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY,
			ZWLR_LAYER_SHELL_V1_LAYER_TOP,
	};
	size_t nlayers = sizeof(layers_above_shell) / sizeof(layers_above_shell[0]);
	struct wc_layer *layer = NULL;
	struct wc_layer *topmost = NULL;
	for (size_t i = 0; i < nlayers; i++) {
		wl_list_for_each_reverse(
				layer, &output->layers[layers_above_shell[i]], link) {
			if (layer->layer_surface->current.keyboard_interactive) {
				topmost = layer;
				break;
			}
		}
		if (topmost != NULL) {
			break;
		}
	}

	wc_seat_set_focus_layer(seat, topmost ? topmost->layer_surface : NULL);
}

static void wc_layer_shell_new_surface(
		struct wl_listener *listener, void *data) {
	struct wc_server *server =
			wl_container_of(listener, server, new_layer_surface);
	struct wlr_layer_surface_v1 *layer_surface = data;
	struct wc_output *active_output = wc_get_active_output(server);
	if (active_output == NULL) {
		wlr_layer_surface_v1_close(layer_surface);
		return;
	}

	if (!layer_surface->output) {
		// If client did not request an output, give them the focused one.
		layer_surface->output = active_output->wlr_output;
	}
	struct wc_output *output = layer_surface->output->data;

	struct wc_layer *layer = calloc(1, sizeof(struct wc_layer));
	layer->server = server;
	layer->layer_surface = layer_surface;

	layer->commit.notify = wc_layer_shell_commit;
	wl_signal_add(&layer_surface->surface->events.commit, &layer->commit);
	layer->map.notify = wc_layer_shell_map;
	wl_signal_add(&layer_surface->events.map, &layer->map);
	layer->unmap.notify = wc_layer_shell_unmap;
	wl_signal_add(&layer_surface->events.unmap, &layer->unmap);
	layer->destroy.notify = wc_layer_shell_destroy;
	wl_signal_add(&layer_surface->events.destroy, &layer->destroy);

	size_t len = sizeof(output->layers) / sizeof(output->layers[0]);
	if (layer_surface->current.layer >= len) {
		wlr_log(WLR_ERROR, "Bad surface layer %d",
				layer_surface->current.layer);
		wlr_layer_surface_v1_close(layer_surface);
		return;
	}
	wl_list_insert(&output->layers[layer_surface->current.layer], &layer->link);

	struct wlr_layer_surface_v1_state old_state = layer_surface->current;
	layer_surface->current = layer_surface->client_pending;
	wc_layer_shell_arrange_layers(output);
	layer_surface->current = old_state;
}

void wc_layers_init(struct wc_server *server) {
	server->layer_shell = wlr_layer_shell_v1_create(server->wl_display);

	server->new_layer_surface.notify = wc_layer_shell_new_surface;
	wl_signal_add(&server->layer_shell->events.new_surface,
			&server->new_layer_surface);
}

void wc_layers_fini(struct wc_server *server) {
	server->layer_shell = NULL;

	wl_list_remove(&server->new_layer_surface.link);
}
