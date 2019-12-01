#define _POSIX_C_SOURCE 200809L
#include "utils/exec.h"

#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>
#include <wlr/util/log.h>

#include <wayland-server.h>

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

struct wl_client *execute(struct wl_display *display, char *command,
		on_destroy_listener on_destroy,
		struct wl_listener *on_destroy_listener) {
	int sockets[2];
	if (socketpair(AF_UNIX, SOCK_STREAM, 0, sockets) != 0) {
		wlr_log(WLR_ERROR, "Failed to create client wayland socket pair");
		abort();
	}
	if (!set_cloexec(sockets[0], true) || !set_cloexec(sockets[1], true)) {
		wlr_log(WLR_ERROR, "Failed to set exec flag for socket");
		abort();
	}
	struct wl_client *client = wl_client_create(display, sockets[0]);
	if (client == NULL) {
		wlr_log(WLR_ERROR, "Could not create startup wl_client");
		abort();
	}
	if (on_destroy != NULL && on_destroy_listener != NULL) {
		on_destroy_listener->notify = on_destroy;
		wl_client_add_destroy_listener(client, on_destroy_listener);
	}

	wlr_log(WLR_INFO, "Executing \"%s\"", command);
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
			execl("/bin/sh", "/bin/sh", "-c", command, NULL);
			wlr_log(WLR_ERROR, "exec failed");
			exit(1);
		}
		exit(0);
	}
	close(sockets[1]);
	return client;
}
