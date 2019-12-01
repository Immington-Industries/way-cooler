#ifndef WC_EXEC_H
#define WC_EXEC_H

#include <wayland-server.h>

typedef void (*on_destroy_listener)(struct wl_listener *listener, void *data);

// Execute a command and pass it the client that this function creates.
struct wl_client *execute(struct wl_display *display, char *command,
		on_destroy_listener on_destroy,
		struct wl_listener *on_destroy_listener);

#endif  // WC_EXEC_H
