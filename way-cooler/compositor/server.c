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
#include "plugins/keybindings.h"

static void startup_command_killed(struct wl_listener *listener, void *data) {
	struct wc_server *server =
			wl_container_of(listener, server, startup_client_destroyed);
	wlr_log(WLR_INFO, "Startup command killed");
	// TODO Something sophisticated - restart the client, shutdown, etc.
}

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

	wc_keybindings_init(server);

	return true;
}

void fini_server(struct wc_server *server) {
	// TODO Why is this segfaulting compositor closing?
	/*
	wc_seat_fini(server);
	wc_output_fini(server);
	wc_inputs_fini(server);
	wc_views_fini(server);
	wc_layers_fini(server);
	wc_cursor_fini(server);

	wlr_screencopy_manager_v1_destroy(server->screencopy_manager);
	wlr_data_device_manager_destroy(server->data_device_manager);
	wlr_xdg_output_manager_v1_destroy(server->xdg_output_manager);

	wc_keybindings_fini(server);
	*/

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
	int sockets[2];
	if (socketpair(AF_UNIX, SOCK_STREAM, 0, sockets) != 0) {
		wlr_log(WLR_ERROR, "Failed to create client wayland socket pair");
		abort();
	}
	if (!set_cloexec(sockets[0], true) || !set_cloexec(sockets[1], true)) {
		wlr_log(WLR_ERROR, "Failed to set exec flag for socket");
		abort();
	}
	server->startup_client = wl_client_create(server->wl_display, sockets[0]);
	if (server->startup_client == NULL) {
		wlr_log(WLR_ERROR, "Could not create startup wl_client");
		abort();
	}
	server->startup_client_destroyed.notify = startup_command_killed;
	wl_client_add_destroy_listener(
			server->startup_client, &server->startup_client_destroyed);

	wlr_log(WLR_INFO, "Executing \"%s\"", server->startup_cmd);
	pid_t pid = fork();
	if (pid < 0) {
		wlr_log(WLR_ERROR, "Failed to fork for startup command");
		abort();
	} else if (pid == 0) {
		/* Child process. Will be used to prevent zombie processes by
		   killing its parent and having init be its new parent.
		*/
		pid = fork();
		if (pid < 0) {
			wlr_log(WLR_ERROR, "Failed to fork for second time");
			abort();
		} else if (pid == 0) {
			if (!set_cloexec(sockets[1], false)) {
				wlr_log(WLR_ERROR,
						"Could not unset close exec flag for forked child");
				abort();
			}
			char wayland_socket_str[16];
			snprintf(wayland_socket_str, sizeof(wayland_socket_str), "%d",
					sockets[1]);
			setenv("WAYLAND_SOCKET", wayland_socket_str, true);
			execl("/bin/sh", "/bin/sh", "-c", server->startup_cmd, NULL);
			wlr_log(WLR_ERROR, "exec failed");
			exit(1);
		}
		exit(0);
	}
	close(sockets[1]);
}
