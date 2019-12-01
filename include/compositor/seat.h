#ifndef WC_SEAT_H
#define WC_SEAT_H

#include <wayland-server.h>
#include <wlr/types/wlr_layer_shell_v1.h>
#include <wlr/types/wlr_surface.h>

#include "server.h"

struct wc_seat {
	struct wc_server *server;
	struct wlr_seat *seat;

	struct wl_listener request_set_cursor;
};

void wc_seat_init(struct wc_server *server);

void wc_seat_fini(struct wc_server *server);

// Updates the seat's focus based on the surface. If surface is NULL the focus
// is cleared.
void wc_seat_update_surface_focus(struct wc_seat *seat,
		struct wlr_surface *surface, double sx, double sy, uint32_t time);

void wc_seat_set_focus_layer(
		struct wc_seat *seat, struct wlr_layer_surface_v1 *layer);

#endif  // WC_SEAT_H
