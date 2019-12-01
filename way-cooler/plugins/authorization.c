#define _POSIX_C_SOURCE 200809L
#include "plugins/authorization.h"

#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

#include <wlr/util/log.h>

#include "compositor/server.h"

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

static void authorized_client_killed(struct wl_listener *listener, void *data) {
	struct wc_authorization *authorization =
			wl_container_of(listener, authorization, client_destroyed);
	wc_authorization_destroy(authorization);
}

void execute_with_authorization(
		struct wc_authorization *authorization, char *command) {
	struct wc_server *server = authorization->plugins->server;
	int sockets[2];
	if (socketpair(AF_UNIX, SOCK_STREAM, 0, sockets) != 0) {
		wlr_log(WLR_ERROR, "Failed to create client wayland socket pair");
		abort();
	}
	if (!set_cloexec(sockets[0], true) || !set_cloexec(sockets[1], true)) {
		wlr_log(WLR_ERROR, "Failed to set exec flag for socket");
		abort();
	}
	authorization->client = wl_client_create(server->wl_display, sockets[0]);
	if (authorization->client == NULL) {
		wlr_log(WLR_ERROR, "Could not create startup wl_client");
		abort();
	}
	authorization->client_destroyed.notify = authorized_client_killed;
	wl_client_add_destroy_listener(
			server->startup_client, &authorization->client_destroyed);

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
}
