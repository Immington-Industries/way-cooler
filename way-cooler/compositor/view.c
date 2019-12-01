#include "compositor/view.h"

#include <assert.h>
#include <stdlib.h>

#include <wayland-server.h>
#include <wlr/types/wlr_output_damage.h>
#include <wlr/types/wlr_surface.h>
#include <wlr/util/log.h>

#include "compositor/cursor.h"
#include "compositor/layer_shell.h"
#include "compositor/output.h"
#include "compositor/seat.h"
#include "compositor/server.h"
#include "compositor/xdg.h"
#include "compositor/xwayland.h"

static bool wc_is_view_at(struct wc_view *view, double lx, double ly,
		double *out_sx, double *out_sy, struct wlr_surface **out_surface) {
	int view_sx = lx - view->geo.x;
	int view_sy = ly - view->geo.y;

	*out_surface = NULL;
	switch (view->surface_type) {
	case WC_XDG:
		*out_surface = wlr_xdg_surface_surface_at(
				view->xdg_surface, view_sx, view_sy, out_sx, out_sy);
		break;
	case WC_XWAYLAND:
		if (view->xwayland_surface->surface != NULL) {
			*out_surface =
					wlr_surface_surface_at(view->xwayland_surface->surface,
							view_sx, view_sy, out_sx, out_sy);
		}
		break;
	}
	return *out_surface != NULL;
}

void wc_view_get_outputs(struct wlr_output_layout *layout, struct wc_view *view,
		struct wlr_output **out_outputs) {
	struct wlr_box geo = {0};
	memcpy(&geo, &view->geo, sizeof(struct wlr_box));

	size_t next_index = 0;
	// top left
	out_outputs[next_index++] =
			wlr_output_layout_output_at(layout, geo.x, geo.y);
	// top right
	out_outputs[next_index++] =
			wlr_output_layout_output_at(layout, geo.x + geo.width, geo.y);
	// bottom left
	out_outputs[next_index++] =
			wlr_output_layout_output_at(layout, geo.x, geo.y + geo.height);
	// bottom right
	out_outputs[next_index++] = wlr_output_layout_output_at(
			layout, geo.x + geo.width, geo.y + geo.height);

	for (size_t i = 1; i < 4; i++) {
		for (size_t j = 0; j < i; j++) {
			if (out_outputs[i] == out_outputs[j]) {
				out_outputs[i] = NULL;
			}
		}
	}
}

struct wlr_surface *wc_view_surface(struct wc_view *view) {
	switch (view->surface_type) {
	case WC_XDG:
		return view->xdg_surface->surface;
	case WC_XWAYLAND:
		return view->xwayland_surface->surface;
	}
	return NULL;
}

void wc_view_move(struct wc_view *view, struct wlr_box geo) {
	struct wc_server *server = view->server;
	struct wlr_surface *focused_surface =
			server->seat->seat->pointer_state.focused_surface;
	struct wlr_surface *surface = wc_view_surface(view);

	if (surface != focused_surface) {
		return;
	}

	struct wc_cursor *cursor = server->cursor;
	struct wlr_cursor *wlr_cursor = cursor->wlr_cursor;

	cursor->cursor_mode = WC_CURSOR_MOVE;
	cursor->grabbed.view = view;
	cursor->grabbed.original_x = wlr_cursor->x - view->geo.x;
	cursor->grabbed.original_y = wlr_cursor->y - view->geo.y;

	cursor->grabbed.original_view_geo.x = view->geo.x;
	cursor->grabbed.original_view_geo.y = view->geo.y;
	cursor->grabbed.original_view_geo.width = geo.width;
	cursor->grabbed.original_view_geo.height = geo.height;
}

void wc_view_resize(struct wc_view *view, struct wlr_box geo, uint32_t edges) {
	struct wc_server *server = view->server;
	struct wc_cursor *cursor = server->cursor;
	struct wlr_cursor *wlr_cursor = cursor->wlr_cursor;
	struct wlr_surface *focused_surface =
			server->seat->seat->pointer_state.focused_surface;
	struct wlr_surface *surface = wc_view_surface(view);

	if (surface != focused_surface) {
		return;
	}

	cursor->cursor_mode = WC_CURSOR_RESIZE;
	cursor->grabbed.view = view;
	cursor->grabbed.original_x = wlr_cursor->x;
	cursor->grabbed.original_y = wlr_cursor->y;

	cursor->grabbed.original_view_geo.x = view->geo.x;
	cursor->grabbed.original_view_geo.y = view->geo.y;
	cursor->grabbed.original_view_geo.width = geo.width;
	cursor->grabbed.original_view_geo.height = geo.height;

	cursor->grabbed.resize_edges = edges;
}

void wc_view_update_geometry(struct wc_view *view, struct wlr_box new_geo) {
	switch (view->surface_type) {
	case WC_XDG:
		view->pending_serial = wlr_xdg_toplevel_set_size(
				view->xdg_surface, new_geo.width, new_geo.height);
		view->is_pending_serial = true;
		break;
	case WC_XWAYLAND:
		view->pending_serial = 1;
		wlr_xwayland_surface_configure(view->xwayland_surface, new_geo.x,
				new_geo.y, new_geo.width, new_geo.height);
		break;
	}

	memcpy(&view->pending_geometry, &new_geo, sizeof(struct wlr_box));
}

void wc_view_damage(struct wc_view *view, pixman_region32_t *damage) {
	struct wlr_output *outputs[4] = {0};
	wc_view_get_outputs(view->server->output_layout, view, outputs);

	// Keep a copy of the damage because otherwise it gets screwed up
	// in the presence of multiple outputs.
	pixman_region32_t damage_copy;
	pixman_region32_init(&damage_copy);
	if (damage != NULL) {
		pixman_region32_copy(&damage_copy, damage);
	}
	struct wlr_surface *surface = NULL;
	switch (view->surface_type) {
	case WC_XDG:
		surface = view->xdg_surface->surface;
		break;
	case WC_XWAYLAND:
		surface = view->xwayland_surface->surface;
		break;
	}
	if (surface == NULL) {
		return;
	}

	for (size_t i = 0; i < 4; i++) {
		struct wlr_output *output = outputs[i];
		struct wlr_box *output_box =
				wlr_output_layout_get_box(view->server->output_layout, output);

		if (output) {
			struct wlr_box view_output_geo = {
					.x = view->geo.x - output_box->x,
					.y = view->geo.y - output_box->y,
					.width = view->geo.width,
					.height = view->geo.height,
			};
			if (damage != NULL) {
				pixman_region32_translate(
						damage, view_output_geo.x, view_output_geo.y);
				wc_output_damage_surface(
						output->data, surface, damage, view_output_geo);
				pixman_region32_copy(damage, &damage_copy);
			} else {
				wc_output_damage_surface(
						output->data, surface, damage, view_output_geo);
			}
		}
	}
	pixman_region32_fini(&damage_copy);
}

void wc_view_damage_whole(struct wc_view *view) {
	wc_view_damage(view, NULL);
}

void wc_view_commit(struct wc_view *view, struct wlr_box geo) {
	if (!view->mapped) {
		return;
	}

	struct wlr_surface *surface = NULL;
	switch (view->surface_type) {
	case WC_XDG:
		surface = view->xdg_surface->surface;
		break;
	case WC_XWAYLAND:
		surface = view->xwayland_surface->surface;
		break;
	}
	if (surface == NULL) {
		return;
	}

	pixman_region32_t damage;
	pixman_region32_init(&damage);
	wlr_surface_get_effective_damage(surface, &damage);
	wc_view_damage(view, &damage);

	bool size_changed = view->geo.width != surface->current.width ||
			view->geo.height != surface->current.height;

	if (size_changed) {
		wc_view_damage_whole(view);
		view->geo.width = surface->current.width;
		view->geo.height = surface->current.height;
		wc_view_damage_whole(view);
	}

	uint32_t pending_serial = view->pending_serial;
	switch (view->surface_type) {
	case WC_XDG:
		if (pending_serial > 0 &&
				pending_serial >= view->xdg_surface->configure_serial) {
			wc_view_damage_whole(view);

			if (view->pending_geometry.x != view->geo.x) {
				view->geo.x = view->pending_geometry.x +
						view->pending_geometry.width - geo.width;
			}
			if (view->pending_geometry.y != view->geo.y) {
				view->geo.y = view->pending_geometry.y +
						view->pending_geometry.height - geo.height;
			}

			wc_view_damage_whole(view);

			if (pending_serial == view->xdg_surface->configure_serial) {
				view->pending_serial = 0;
				view->is_pending_serial = false;
			}
		}
		break;
	case WC_XWAYLAND:
		if (pending_serial > 0) {
			wc_view_damage_whole(view);

			if (view->pending_geometry.x != view->geo.x) {
				view->geo.x = view->pending_geometry.x +
						view->pending_geometry.width - geo.width;
			}
			if (view->pending_geometry.y != view->geo.y) {
				view->geo.y = view->pending_geometry.y +
						view->pending_geometry.height - geo.height;
			}

			wc_view_damage_whole(view);
			view->pending_serial = 0;
		}
		break;
	}
	pixman_region32_fini(&damage);
}

struct wc_view *wc_view_at(struct wc_server *server, double lx, double ly,
		double *out_sx, double *out_sy, struct wlr_surface **out_surface) {
	assert(out_surface != NULL);

	struct wc_view *view;
	wl_list_for_each(view, &server->views, link) {
		if (wc_is_view_at(view, lx, ly, out_sx, out_sy, out_surface)) {
			return view;
		}
	}
	return NULL;
}

void wc_focus_view(struct wc_view *view) {
	assert(view != NULL);

	struct wc_server *server = view->server;
	struct wlr_surface *surface = wc_view_surface(view);
	struct wlr_seat *seat = server->seat->seat;
	struct wlr_surface *prev_surface = seat->keyboard_state.focused_surface;
	if (prev_surface == surface) {
		return;
	}

	if (prev_surface && wlr_surface_is_xdg_surface(prev_surface)) {
		struct wlr_xdg_surface *previous =
				wlr_xdg_surface_from_wlr_surface(prev_surface);
		wlr_xdg_toplevel_set_activated(previous, false);
	}

	/* Move the view to the front */
	wl_list_remove(&view->link);
	wl_list_insert(&server->views, &view->link);
	wc_view_damage_whole(view);

	switch (view->surface_type) {
	case WC_XDG:
		wlr_xdg_toplevel_set_activated(view->xdg_surface, true);
		break;
	case WC_XWAYLAND:
		wlr_xwayland_surface_activate(view->xwayland_surface, true);
		break;
	}

	struct wlr_keyboard *keyboard = wlr_seat_get_keyboard(seat);
	if (keyboard != NULL) {
		wlr_seat_keyboard_notify_enter(seat, surface, keyboard->keycodes,
				keyboard->num_keycodes, &keyboard->modifiers);
	}
}

void wc_views_init(struct wc_server *server) {
	wl_list_init(&server->views);
	wc_xdg_init(server);
}

void wc_views_fini(struct wc_server *server) {
	struct wc_view *view;
	struct wc_view *temp;
	wl_list_for_each_safe(view, temp, &server->views, link) {
		switch (view->surface_type) {
		case WC_XDG:
			wc_xdg_surface_destroy(&view->destroy, NULL);
			break;
		case WC_XWAYLAND:
			wc_xwayland_surface_destroy(&view->destroy, NULL);
			break;
		}
	}

	wc_xdg_fini(server);
}

void wc_view_for_each_surface(struct wc_view *view,
		wlr_surface_iterator_func_t iterator, void *data) {
	switch (view->surface_type) {
	case WC_XDG:
		wlr_xdg_surface_for_each_surface(view->xdg_surface, iterator, data);
		break;
	case WC_XWAYLAND: {
		struct wlr_xwayland_surface *xwayland_surface = view->xwayland_surface;
		iterator(xwayland_surface->surface, 0, 0, data);
		break;
	}
	}
}
