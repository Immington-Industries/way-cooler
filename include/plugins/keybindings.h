#ifndef WC_KEYBINDINGS_H
#define WC_KEYBINDINGS_H

#include <stdint.h>
#include <wayland-server.h>

#include "compositor/server.h"
#include "utils/xkb_hash_set.h"

#define KEYBINDINGS_VERSION 1

struct wc_keybindings {
	struct wl_list link;  // wc_server.keybinders
	struct wc_server *server;

	struct xkb_hash_set *registered_keys;

	struct wl_resource *resource;
	struct wl_client *client;
};

void wc_keybindings_init(struct wc_server *server);

void wc_keybindings_fini(struct wc_server *server);

/*
 * Checks if the key is registered as a keybinding and, if so, sends it to the
 * registered keybinding client(s).
 *
 * If the key is registered by at least one client true is returned.
 *
 * Mods is expected to be all mods that are either depressed, latched, or
 * locked.
 */
bool wc_keybindings_notify_key_if_registered(struct wc_server *server,
		uint32_t key_code, xkb_mod_mask_t key_mask, bool pressed,
		uint32_t time);

/*
 * Clears the stored keybindings, meaning those keys will no longer be filtered
 * from other clients.
 */

void wc_keybindings_clear_keys(struct wc_keybindings *keybindings);

#endif  // WC_KEYBINDINGS_H
