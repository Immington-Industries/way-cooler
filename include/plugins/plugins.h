#ifndef WC_PLUGINS_H
#define WC_PLUGINS_H

#include <stdbool.h>

#include <wayland-server.h>

struct wc_plugins {
	struct wc_server *server;

	struct wl_global *keybindings_global;
	struct wl_list keybinders;
};

bool wc_plugins_init(struct wc_server *server);
void wc_plugins_fini(struct wc_server *server);

#endif  // WC_PLUGINS_H
