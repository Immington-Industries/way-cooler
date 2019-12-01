#define _POSIX_C_SOURCE 200809L

#include <getopt.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wordexp.h>

#include <wlr/backend.h>
#include <wlr/types/wlr_data_device.h>
#include <wlr/util/log.h>

#include "compositor/server.h"

const char *WC_HELP_MESSAGE =
		"Usage: %s [OPTION] startup_command\n"
		"\n"
		"  -c <command>           Execute the command after startup.\n"
		"  -h                     Show help message and quit.\n"
		"  -d                     Turn on debugging"
		"\n";

const char *WC_GETOPT_OPTIONS =
#ifdef __GNUC__
		"+"
#endif
		"hc:d";

const char *WC_BINARY_PATH = NULL;

void print_usage(void) {
	printf(WC_HELP_MESSAGE, WC_BINARY_PATH);
}

int main(int argc, char *argv[]) {
	WC_BINARY_PATH = argv[0];
	wlr_log_init(WLR_DEBUG, NULL);
	char *startup_cmd = NULL;

	int c;
	while ((c = getopt(argc, argv, WC_GETOPT_OPTIONS)) != -1) {
		switch (c) {
		case 'd':
			WC_DEBUG = 1;
			break;
		case 'c':
			if (startup_cmd != NULL) {
				free(startup_cmd);
			}
			startup_cmd = strdup(optarg);
			break;
		case 'h':
		default:
			print_usage();
			goto fail;
		}
	}
	if (optind < argc) {
		print_usage();
		goto fail;
	}

	struct wc_server server = {0};
	if (!init_server(&server)) {
		wlr_log(WLR_ERROR, "Could not initialize Wayland resources");
		goto fail;
	}
	wlr_log(WLR_INFO, "Running Wayland compositor on WAYLAND_DISPLAY=%s",
			server.wayland_socket);
	if (!wlr_backend_start(server.backend)) {
		wlr_backend_destroy(server.backend);
		wl_display_destroy(server.wl_display);
		goto fail;
	}
	setenv("WAYLAND_DISPLAY", server.wayland_socket, true);

	server.startup_cmd = startup_cmd;

	wl_display_run(server.wl_display);
	fini_server(&server);

	return 0;

fail:
	free(startup_cmd);
	exit(1);
}
