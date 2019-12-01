#ifndef WC_XWAYLAND_H
#define WC_XWAYLAND_H

#include "server.h"

void wc_xwayland_init(struct wc_server *server);

void wc_xwayland_fini(struct wc_server *server);

void wc_xwayland_surface_destroy(struct wl_listener *listener, void *data);

#endif  // WC_XWAYLAND_H
