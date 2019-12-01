#define _POSIX_C_SOURCE 200112L
#include "compositor/xwayland.h"

#include <stdlib.h>
#include <unistd.h>

#include <wlr/util/log.h>

#include "compositor/cursor.h"
#include "compositor/seat.h"
#include "compositor/server.h"
#include "compositor/view.h"

void wc_xwayland_surface_destroy(struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, destroy);

	wl_list_remove(&view->link);

	wl_list_remove(&view->map.link);
	wl_list_remove(&view->unmap.link);
	wl_list_remove(&view->configure.link);
	wl_list_remove(&view->request_move.link);
	wl_list_remove(&view->request_resize.link);
	wl_list_remove(&view->destroy.link);

	free(view);
}

static void wc_xwayland_request_configure(
		struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, configure);

	struct wlr_xwayland_surface_configure_event *event = data;

	view->geo.x = event->x;
	view->geo.y = event->y;
	view->geo.width = event->width;
	view->geo.height = event->height;

	wlr_xwayland_surface_configure(view->xwayland_surface, event->x, event->y,
			event->width, event->height);
}

static void wc_xwayland_commit(struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, commit);
	struct wlr_xwayland_surface *xwayland_surface = view->xwayland_surface;

	struct wlr_box size = {
			.x = view->geo.x,
			.y = view->geo.y,
			.width = xwayland_surface->width,
			.height = xwayland_surface->height,
	};

	wc_view_commit(view, size);
}

static void wc_xwayland_surface_map(struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, map);
	struct wlr_xwayland_surface *surface = data;

	view->mapped = true;
	wc_focus_view(view);

	view->geo.width = surface->width;
	view->geo.height = surface->height;

	view->commit.notify = wc_xwayland_commit;
	wl_signal_add(
			&view->xwayland_surface->surface->events.commit, &view->commit);

	wc_view_damage_whole(view);
}

static void wc_xwayland_surface_unmap(
		struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, unmap);
	view->mapped = false;

	wl_list_remove(&view->commit.link);

	wc_view_damage_whole(view);
}

static void wc_xwayland_request_move(struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, request_move);

	struct wlr_box geo = {
			.x = view->geo.x,
			.y = view->geo.y,
			.width = view->xwayland_surface->width,
			.height = view->xwayland_surface->height,
	};

	wc_view_move(view, geo);
}

static void wc_xwayland_request_resize(
		struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, request_resize);
	struct wlr_xwayland_resize_event *event = data;

	struct wlr_box geo = {
			.x = view->geo.x,
			.y = view->geo.y,
			.width = view->xwayland_surface->width,
			.height = view->xwayland_surface->height,
	};

	wc_view_resize(view, geo, event->edges);
}

static void wc_xwayland_new_surface(struct wl_listener *listener, void *data) {
	struct wc_server *server =
			wl_container_of(listener, server, new_xwayland_surface);
	struct wlr_xwayland_surface *xwayland_surface = data;

	struct wc_view *view = calloc(1, sizeof(struct wc_view));
	view->server = server;
	view->xwayland_surface = xwayland_surface;
	view->surface_type = WC_XWAYLAND;

	view->map.notify = wc_xwayland_surface_map;
	view->unmap.notify = wc_xwayland_surface_unmap;
	view->configure.notify = wc_xwayland_request_configure;
	view->request_move.notify = wc_xwayland_request_move;
	view->request_resize.notify = wc_xwayland_request_resize;
	view->destroy.notify = wc_xwayland_surface_destroy;

	wl_signal_add(&xwayland_surface->events.map, &view->map);
	wl_signal_add(&xwayland_surface->events.unmap, &view->unmap);
	wl_signal_add(
			&xwayland_surface->events.request_configure, &view->configure);
	wl_signal_add(&xwayland_surface->events.request_move, &view->request_move);
	wl_signal_add(
			&xwayland_surface->events.request_resize, &view->request_resize);
	wl_signal_add(&xwayland_surface->events.destroy, &view->destroy);

	wl_list_insert(&server->views, &view->link);
}

static void wc_xwayland_ready(struct wl_listener *listener, void *data) {
	struct wc_server *server =
			wl_container_of(listener, server, xwayland_ready);
	if (server->startup_cmd != NULL) {
		// NOTE Executed here so that DISPLAY is correct for the client
		wc_server_execute_startup_command(server);
	}
}

void wc_xwayland_init(struct wc_server *server) {
	server->xwayland =
			wlr_xwayland_create(server->wl_display, server->compositor, false);

	wl_signal_add(&server->xwayland->events.new_surface,
			&server->new_xwayland_surface);
	wl_signal_add(&server->xwayland->events.ready, &server->xwayland_ready);

	server->new_xwayland_surface.notify = wc_xwayland_new_surface;
	server->xwayland_ready.notify = wc_xwayland_ready;

	setenv("DISPLAY", server->xwayland->display_name, true);

	if (server->xwayland == NULL) {
		abort();
	}
}

void wc_xwayland_fini(struct wc_server *server) {
	wlr_xwayland_destroy(server->xwayland);
	server->xwayland = NULL;

	wl_list_remove(&server->new_xwayland_surface.link);
}
