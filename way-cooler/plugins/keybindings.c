#include "plugins/keybindings.h"

#include <stdint.h>
#include <stdlib.h>
#include <wayland-server.h>
#include <wlr/util/log.h>

#include "compositor/seat.h"
#include "compositor/server.h"
#include "utils/xkb_hash_set.h"
#include "way-cooler-keybindings-unstable-v1-protocol.h"

static void register_key(struct wl_client *client, struct wl_resource *resource,
		uint32_t key, uint32_t mods) {
	struct wc_keybindings *keybindings = wl_resource_get_user_data(resource);
	struct xkb_hash_set *registered_keys = keybindings->registered_keys;

	xkb_hash_set_add_entry(registered_keys, key, mods);
}

static void clear_keys(struct wl_client *client, struct wl_resource *resource) {
	struct wc_keybindings *keybindings = wl_resource_get_user_data(resource);
	wc_keybindings_clear_keys(keybindings);
}

static const struct zway_cooler_keybindings_interface keybindings_impl = {
		.register_key = register_key,
		.clear_keys = clear_keys,
};

static void keybindings_handle_resource_destroy(struct wl_resource *resource) {
	struct wc_keybindings *keybindings = wl_resource_get_user_data(resource);

	if (keybindings->resource == resource) {
		wl_list_remove(&keybindings->link);
		xkb_hash_set_destroy(keybindings->registered_keys);
		free(keybindings);
	}
}

static void keybindings_bind(
		struct wl_client *client, void *data, uint32_t version, uint32_t id) {
	struct wc_server *server = data;

	struct wl_resource *resource = wl_resource_create(
			client, &zway_cooler_keybindings_interface, version, id);
	struct wc_keybindings *keybindings =
			calloc(1, sizeof(struct wc_keybindings));
	struct xkb_hash_set *registered_keys = xkb_hash_set_create();
	if (resource == NULL || keybindings == NULL || registered_keys == NULL) {
		wl_client_post_no_memory(client);
		return;
	}

	keybindings->registered_keys = registered_keys;
	keybindings->server = server;
	keybindings->client = client;
	keybindings->resource = resource;
	wl_resource_set_implementation(resource, &keybindings_impl, keybindings,
			keybindings_handle_resource_destroy);
	wl_resource_set_user_data(resource, keybindings);

	wl_list_insert(&server->keybinders, &keybindings->link);
}

void wc_keybindings_init(struct wc_server *server) {
	wl_list_init(&server->keybinders);
	server->keybindings_global = wl_global_create(server->wl_display,
			&zway_cooler_keybindings_interface, KEYBINDINGS_VERSION, server,
			keybindings_bind);
}

void wc_keybindings_fini(struct wc_server *server) {
	wl_global_destroy(server->keybindings_global);

	struct wc_keybindings *keybindings, *temp;
	wl_list_for_each_safe(keybindings, temp, &server->keybinders, link) {
		xkb_hash_set_destroy(keybindings->registered_keys);
		free(keybindings);
	}
}

void wc_keybindings_clear_keys(struct wc_keybindings *keybindings) {
	xkb_hash_set_clear(keybindings->registered_keys);
}

bool wc_keybindings_notify_key_if_registered(struct wc_server *server,
		uint32_t key_code, xkb_mod_mask_t key_mask, bool pressed,
		uint32_t time) {
	enum zway_cooler_keybindings_key_state press_state = pressed ?
			ZWAY_COOLER_KEYBINDINGS_KEY_STATE_PRESSED :
			ZWAY_COOLER_KEYBINDINGS_KEY_STATE_RELEASED;
	bool seen_once = false;
	struct wc_keybindings *keybindings, *temp;
	wl_list_for_each_safe(keybindings, temp, &server->keybinders, link) {
		if (keybindings->resource == NULL) {
			continue;
		}

		struct xkb_hash_set *registered_keys = keybindings->registered_keys;

		bool present =
				xkb_hash_set_get_entry(registered_keys, key_code, key_mask);

		if (present) {
			zway_cooler_keybindings_send_key(keybindings->resource, time,
					key_code, press_state, key_mask);
			seen_once = true;
		}
	}
	return seen_once;
}
