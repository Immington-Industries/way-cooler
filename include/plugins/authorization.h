#ifndef WC_AUTHORIZATION_H
#define WC_AUTHORIZATION_H

#include <stdlib.h>

#include <wayland-server.h>

#include "plugins/plugins.h"

enum wc_permissions {
	KEYBINDINGS = 1 << 0,
};

struct wc_authorization {
	struct wl_list link;  // wc_server.authorizations
	struct wc_plugins *plugins;

	// Bitfield of permissions authorized for this client
	enum wc_permissions permissions;  // wc_permissions

	struct wl_client *client;
	struct wl_listener client_destroyed;
};

// Create a new authorization with some permissions.
struct wc_authorization *wc_authorization_create(
		struct wc_plugins *plugins, enum wc_permissions permissions);

/* Removes the authorization of a client. This removes all permissions and
 * immediately kills the client regardless if they had connected at all.
 */
void wc_authorization_destroy(struct wc_authorization *authorization);

// Executes a command with a certain level of authorization.
void execute_with_authorization(
		struct wc_authorization *authorization, char *command);

#endif  // WC_AUTHORIZATION_H
