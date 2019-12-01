#ifndef WC_XDG_H
#define WC_XDG_H

#include "server.h"

void wc_xdg_init(struct wc_server *server);

void wc_xdg_fini(struct wc_server *server);

void wc_xdg_surface_destroy(struct wl_listener *listener, void *data);

#endif  // WC_XDG_H
