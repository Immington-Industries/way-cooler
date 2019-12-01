#define _POSIX_C_SOURCE 200809L

#include "compositor/server.h"

#include <fcntl.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

#include <wayland-server.h>
#include <wlr/backend.h>
#include <wlr/render/wlr_renderer.h>
#include <wlr/types/wlr_compositor.h>
#include <wlr/types/wlr_cursor.h>
#include <wlr/types/wlr_data_device.h>
#include <wlr/types/wlr_output.h>
#include <wlr/types/wlr_output_layout.h>
#include <wlr/types/wlr_screencopy_v1.h>
#include <wlr/types/wlr_xcursor_manager.h>
#include <wlr/util/log.h>

#include "compositor/cursor.h"
#include "compositor/input.h"
#include "compositor/layer_shell.h"
#include "compositor/output.h"
#include "compositor/seat.h"
#include "compositor/view.h"
#include "compositor/xwayland.h"
#include "plugins/plugins.h"

bool init_server(struct wc_server *server) {
	if (server == NULL) {
		return false;
	}

	server->wl_display = wl_display_create();
	server->wayland_socket = wl_display_add_socket_auto(server->wl_display);
	if (!server->wayland_socket) {
		wlr_backend_destroy(server->backend);
		return false;
	}

	server->backend = wlr_backend_autocreate(server->wl_display, NULL);
	server->renderer = wlr_backend_get_renderer(server->backend);
	wlr_renderer_init_wl_display(server->renderer, server->wl_display);
	server->compositor =
			wlr_compositor_create(server->wl_display, server->renderer);
	if (server->compositor == NULL) {
		return false;
	}

	server->screencopy_manager =
			wlr_screencopy_manager_v1_create(server->wl_display);
	server->data_device_manager =
			wlr_data_device_manager_create(server->wl_display);

	wc_xwayland_init(server);
	wc_seat_init(server);
	wc_output_init(server);
	wc_inputs_init(server);
	wc_views_init(server);
	wc_layers_init(server);
	wc_cursor_init(server);

	// XXX This must be initialized after the output layout
	server->xdg_output_manager = wlr_xdg_output_manager_v1_create(
			server->wl_display, server->output_layout);

	return wc_plugins_init(server);
}

void fini_server(struct wc_server *server) {
	wc_plugins_fini(server);

	wc_xwayland_fini(server);
	wl_display_destroy_clients(server->wl_display);
	wl_display_destroy(server->wl_display);
}

static bool set_cloexec(int fd, bool cloexec) {
	int flags = fcntl(fd, F_GETFD);
	if (flags == -1) {
		goto failed;
	}
	if (cloexec) {
		flags = flags | FD_CLOEXEC;
	} else {
		flags = flags & ~FD_CLOEXEC;
	}
	if (fcntl(fd, F_SETFD, flags) == -1) {
		goto failed;
	}
	return true;
failed:
	wlr_log(WLR_ERROR, "fcntl failed");
	return false;
}

void wc_server_execute_startup_command(struct wc_server *server) {
	execute(server->display, server->startup_cmd, NULL, NULL);
}
