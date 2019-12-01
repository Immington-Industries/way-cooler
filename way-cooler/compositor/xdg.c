#include "compositor/xdg.h"

#include <stdint.h>
#include <stdlib.h>

#include <wayland-server.h>
#include <wlr/types/wlr_xdg_shell.h>
#include <wlr/util/log.h>

#include "compositor/cursor.h"
#include "compositor/output.h"
#include "compositor/seat.h"
#include "compositor/server.h"
#include "compositor/view.h"

static void wc_xdg_surface_map(struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, map);
	view->mapped = true;
	wc_focus_view(view);

	struct wlr_xdg_surface *surface = view->xdg_surface;
	struct wlr_box box = {0};
	wlr_xdg_surface_get_geometry(surface, &box);
	memcpy(&view->geo, &box, sizeof(struct wlr_box));

	wc_view_damage_whole(view);
}

static void wc_xdg_surface_unmap(struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, unmap);
	view->mapped = false;

	wc_view_damage_whole(view);
}

static void wc_xdg_surface_commit(struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, commit);
	struct wlr_xdg_surface *xdg_surface = view->xdg_surface;

	struct wlr_box geo = {0};
	wlr_xdg_surface_get_geometry(xdg_surface, &geo);

	wc_view_commit(view, geo);
}

void wc_xdg_surface_destroy(struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, destroy);
	wl_list_remove(&view->link);

	wl_list_remove(&view->map.link);
	wl_list_remove(&view->unmap.link);
	wl_list_remove(&view->commit.link);
	wl_list_remove(&view->request_move.link);
	wl_list_remove(&view->request_resize.link);
	wl_list_remove(&view->destroy.link);

	free(view);
}

static void wc_xdg_toplevel_request_move(
		struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, request_move);

	struct wlr_box geo;
	wlr_xdg_surface_get_geometry(view->xdg_surface, &geo);

	wc_view_move(view, geo);
}

static void wc_xdg_toplevel_request_resize(
		struct wl_listener *listener, void *data) {
	struct wc_view *view = wl_container_of(listener, view, request_resize);
	struct wlr_xdg_toplevel_resize_event *event = data;

	struct wlr_box geo;
	wlr_xdg_surface_get_geometry(view->xdg_surface, &geo);

	wc_view_resize(view, geo, event->edges);
}

static void wc_xdg_new_surface(struct wl_listener *listener, void *data) {
	struct wc_server *server =
			wl_container_of(listener, server, new_xdg_surface);
	struct wlr_xdg_surface *xdg_surface = data;
	if (xdg_surface->role != WLR_XDG_SURFACE_ROLE_TOPLEVEL) {
		return;
	}

	struct wc_view *view = calloc(1, sizeof(struct wc_view));
	view->server = server;
	view->xdg_surface = xdg_surface;
	view->surface_type = WC_XDG;

	view->map.notify = wc_xdg_surface_map;
	view->unmap.notify = wc_xdg_surface_unmap;
	view->commit.notify = wc_xdg_surface_commit;
	view->destroy.notify = wc_xdg_surface_destroy;

	wl_signal_add(&xdg_surface->events.map, &view->map);
	wl_signal_add(&xdg_surface->events.unmap, &view->unmap);
	wl_signal_add(&xdg_surface->surface->events.commit, &view->commit);
	wl_signal_add(&xdg_surface->events.destroy, &view->destroy);

	struct wlr_xdg_toplevel *toplevel = xdg_surface->toplevel;
	view->request_move.notify = wc_xdg_toplevel_request_move;
	wl_signal_add(&toplevel->events.request_move, &view->request_move);
	view->request_resize.notify = wc_xdg_toplevel_request_resize;
	wl_signal_add(&toplevel->events.request_resize, &view->request_resize);

	wl_list_insert(&server->views, &view->link);
}

void wc_xdg_init(struct wc_server *server) {
	server->xdg_shell = wlr_xdg_shell_create(server->wl_display);
	server->new_xdg_surface.notify = wc_xdg_new_surface;
	wl_signal_add(
			&server->xdg_shell->events.new_surface, &server->new_xdg_surface);
}

void wc_xdg_fini(struct wc_server *server) {
	server->xdg_shell = NULL;

	wl_list_remove(&server->new_xdg_surface.link);
}
