#ifndef WC_SERVER_H
#define WC_SERVER_H

#include <wayland-server.h>
#include <wlr/backend.h>
#include <wlr/render/wlr_renderer.h>
#include <wlr/types/wlr_compositor.h>
#include <wlr/types/wlr_cursor.h>
#include <wlr/types/wlr_layer_shell_v1.h>
#include <wlr/types/wlr_output.h>
#include <wlr/types/wlr_output_layout.h>
#include <wlr/types/wlr_screencopy_v1.h>
#include <wlr/types/wlr_seat.h>
#include <wlr/types/wlr_xcursor_manager.h>
#include <wlr/types/wlr_xdg_output_v1.h>
#include <wlr/xwayland.h>

int WC_DEBUG;

struct wc_server {
	const char *wayland_socket;
	struct wl_display *wl_display;
	struct wlr_backend *backend;
	struct wlr_renderer *renderer;
	struct wlr_compositor *compositor;

	const char *startup_cmd;
	struct wl_client *startup_client;

	struct wlr_xcursor_manager *xcursor_mgr;
	struct wc_cursor *cursor;

	struct wc_seat *seat;

	struct wl_list keyboards;
	struct wl_list pointers;
	struct wl_listener new_input;

	struct wlr_output_layout *output_layout;
	struct wc_output *active_output;
	struct wl_list outputs;
	struct wl_listener new_output;

	struct wl_list views;

	struct wlr_xwayland *xwayland;
	struct wl_listener new_xwayland_surface;
	struct wl_listener xwayland_ready;

	struct wlr_xdg_shell *xdg_shell;
	struct wl_listener new_xdg_surface;

	struct wlr_layer_shell_v1 *layer_shell;
	struct wl_listener new_layer_surface;

	struct wlr_screencopy_manager_v1 *screencopy_manager;
	struct wlr_data_device_manager *data_device_manager;
	struct wlr_xdg_output_manager_v1 *xdg_output_manager;

	struct wc_plugins *plugins;
};

bool init_server(struct wc_server *server);
void fini_server(struct wc_server *server);
void wc_server_execute_startup_command(struct wc_server *server);

#endif  // WC_SERVER_H
