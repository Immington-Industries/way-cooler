#ifndef LAYER_SHELL_H
#define LAYER_SHELL_H

#include <wayland-server.h>

#include "server.h"

struct wc_layer {
	struct wl_list link;
	struct wc_server *server;

	struct wlr_layer_surface_v1 *layer_surface;
	struct wlr_box geo;
	bool mapped;
	enum zwlr_layer_shell_v1_layer layer;

	struct wl_listener commit;
	struct wl_listener map;
	struct wl_listener unmap;
	struct wl_listener destroy;
};

void wc_layers_init(struct wc_server *server);

void wc_layers_fini(struct wc_server *server);

void wc_layer_shell_destroy(struct wl_listener *listener, void *data);

// Arrange the layer shells on this output.
void wc_layer_shell_arrange_layers(struct wc_output *output);

#endif  // LAYER_SHELL_H
