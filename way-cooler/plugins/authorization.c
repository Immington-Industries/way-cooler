#include "plugins/authorization.h"

#include "compositor/server.h"
#include "utils/exec.h"

struct wc_authorization *wc_authorization_create(
		struct wc_plugins *plugins, enum wc_permissions permissions) {
	struct wc_authorization *authorization =
			calloc(1, sizeof(struct wc_authorization));
	if (authorization == NULL) {
		return NULL;
	}
	authorization->plugins = plugins;
	authorization->permissions = permissions;
	wl_list_insert(&plugins->authorizations, &authorization->link);
	return authorization;
}

void wc_authorization_destroy(struct wc_authorization *authorization) {
	if (authorization == NULL) {
		return;
	}
	wl_list_remove(&authorization->link);
	if (authorization->client) {
		// TODO Will this recurse?
		wl_client_destroy(authorization->client);
	}
	free(authorization);
}

static void authorized_client_killed(struct wl_listener *listener, void *data) {
	struct wc_authorization *authorization =
			wl_container_of(listener, authorization, client_destroyed);
	wc_authorization_destroy(authorization);
}

void execute_with_authorization(
		struct wc_authorization *authorization, char *command) {
	struct wc_server *server = authorization->plugins->server;
	execute(server->wl_display, command, authorized_client_killed,
			&authorization->client_destroyed);
}
