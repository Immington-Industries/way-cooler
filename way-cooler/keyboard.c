#include "keyboard.h"

#include <stdlib.h>
#include <unistd.h>

#include <wayland-server.h>
#include <wlr/backend/multi.h>
#include <wlr/backend/session.h>
#include <wlr/types/wlr_input_device.h>
#include <wlr/util/log.h>
#include <xkbcommon/xkbcommon.h>

#include "keybindings.h"
#include "seat.h"

static bool wc_keyboard_mod_is_active(
		struct wc_keyboard *keyboard, const char *mod_name) {
	struct xkb_state *state = keyboard->device->keyboard->xkb_state;
	return xkb_state_mod_name_is_active(
			state, mod_name, XKB_STATE_MODS_DEPRESSED);
}

static void wc_keyboard_on_key(struct wl_listener *listener, void *data) {
	struct wc_keyboard *keyboard = wl_container_of(listener, keyboard, key);
	struct wc_server *server = keyboard->server;
	struct wlr_seat *seat = server->seat->seat;
	struct wlr_event_keyboard_key *event = data;

	uint32_t keycode = event->keycode + 8;
	const xkb_keysym_t *syms;
	int nsyms = xkb_state_key_get_syms(
			keyboard->device->keyboard->xkb_state, keycode, &syms);

	bool handled = false;
	for (int i = 0; i < nsyms; i++) {
		xkb_keysym_t keysym = syms[i];
		if (keysym >= XKB_KEY_XF86Switch_VT_1 &&
				keysym <= XKB_KEY_XF86Switch_VT_12) {
			handled = true;
			if (wlr_backend_is_multi(server->backend)) {
				struct wlr_session *session =
						wlr_backend_get_session(server->backend);
				if (session) {
					xkb_keysym_t vt = keysym - XKB_KEY_XF86Switch_VT_1 + 1;
					wlr_session_change_vt(session, vt);
				}
			}
		}

		switch (keysym) {
		case XKB_KEY_Escape:
			if (wc_keyboard_mod_is_active(keyboard, "Shift") &&
					wc_keyboard_mod_is_active(keyboard, "Control")) {
				wl_display_terminate(server->wl_display);
				handled = true;
				break;
			}
		}
	}
	if (!handled) {
		struct wlr_keyboard_modifiers *keyboard_modifiers =
				&keyboard->device->keyboard->modifiers;
		xkb_mod_mask_t modifiers = keyboard_modifiers->depressed |
				keyboard_modifiers->latched | keyboard_modifiers->locked;

		bool pressed = event->state == WLR_KEY_PRESSED;
		handled = wc_keybindings_notify_key_if_registered(
				server, keycode, modifiers, pressed, event->time_msec);
	}

	if (!handled) {
		wlr_seat_set_keyboard(seat, keyboard->device);
		wlr_seat_keyboard_notify_key(
				seat, event->time_msec, event->keycode, event->state);
	}
}

static void wc_keyboard_on_modifiers(struct wl_listener *listener, void *data) {
	struct wc_keyboard *keyboard =
			wl_container_of(listener, keyboard, modifiers);
	struct wlr_seat *seat = keyboard->server->seat->seat;

	wlr_seat_set_keyboard(seat, keyboard->device);
	wlr_seat_keyboard_notify_modifiers(
			seat, &keyboard->device->keyboard->modifiers);
}

static void wc_keyboard_removed(struct wl_listener *listener, void *data) {
	struct wc_keyboard *keyboard = wl_container_of(listener, keyboard, destroy);
	wlr_log(WLR_INFO, "Keyboard removed: %p", keyboard->device);
	wl_list_remove(&keyboard->link);

	wl_list_remove(&keyboard->key.link);
	wl_list_remove(&keyboard->modifiers.link);
	wl_list_remove(&keyboard->destroy.link);
	free(keyboard);
}

void wc_new_keyboard(
		struct wc_server *server, struct wlr_input_device *device) {
	wlr_log(WLR_INFO, "New keyboard detected: %p", device);

	wlr_seat_set_keyboard(server->seat->seat, device);

	struct wc_keyboard *keyboard = calloc(1, sizeof(struct wc_keyboard));
	keyboard->server = server;
	keyboard->device = device;

	/* We need to prepare an XKB keymap and assign it to the keyboard. This
	 * assumes the defaults (i.e. uses the XKB_DEFAULT_* environment variables).
	 */
	struct xkb_rule_names rules = {0};
	struct xkb_context *context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
	struct xkb_keymap *keymap = xkb_map_new_from_names(
			context, &rules, XKB_KEYMAP_COMPILE_NO_FLAGS);

	wlr_keyboard_set_keymap(device->keyboard, keymap);
	xkb_keymap_unref(keymap);
	xkb_context_unref(context);

	wlr_keyboard_set_repeat_info(device->keyboard, 25, 600);

	keyboard->key.notify = wc_keyboard_on_key;
	keyboard->modifiers.notify = wc_keyboard_on_modifiers;
	keyboard->destroy.notify = wc_keyboard_removed;

	wl_signal_add(&device->keyboard->events.key, &keyboard->key);
	wl_signal_add(&device->keyboard->events.modifiers, &keyboard->modifiers);
	wl_signal_add(&device->events.destroy, &keyboard->destroy);

	wl_list_insert(&server->keyboards, &keyboard->link);
}

void wc_keyboards_init(struct wc_server *server) {
	wl_list_init(&server->keyboards);
}

void wc_keyboards_fini(struct wc_server *server) {
	struct wc_keyboard *keyboard;
	struct wc_keyboard *temp;
	wl_list_for_each_safe(keyboard, temp, &server->keyboards, link) {
		wc_keyboard_removed(&keyboard->destroy, NULL);
	}
}
