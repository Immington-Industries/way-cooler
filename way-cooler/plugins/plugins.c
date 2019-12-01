#include "plugins/plugins.h"

#include <stdbool.h>
#include <stdlib.h>

#include "compositor/server.h"
#include "plugins/keybindings.h"

bool wc_plugins_init(struct wc_server *server) {
	server->plugins = calloc(1, sizeof(struct wc_plugins));
	if (server->plugins == NULL) {
		return false;
	}
	server->plugins->server = server;

	wc_keybindings_init(server->plugins);
	return true;
}

void wc_plugins_fini(struct wc_server *server) {
	wc_keybindings_fini(server->plugins);
}
