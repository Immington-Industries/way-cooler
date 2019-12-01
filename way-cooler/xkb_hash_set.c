#include "xkb_hash_set.h"

#include <stdlib.h>
#include <string.h>

#include <xcb/xcb.h>

struct xkb_hash_set *xkb_hash_set_create(void) {
	return calloc(1, sizeof(struct xkb_hash_set));
}

void xkb_hash_set_destroy(struct xkb_hash_set *hash_set) {
	if (hash_set == NULL) {
		return;
	}
	xkb_hash_set_clear(hash_set);
	free(hash_set);
}

void xkb_hash_set_clear(struct xkb_hash_set *hash_set) {
	const size_t hash_set_size =
			sizeof(hash_set->set) / sizeof(hash_set->set[0]);
	for (size_t i = 0; i < hash_set_size; i++) {
		struct hash_entry *entry = hash_set->set[i].next;
		struct hash_entry *next;
		while (entry != NULL) {
			next = entry->next;
			free(entry);
			entry = next;
		}
	}
	memset(&hash_set->set, 0, sizeof(hash_set->set));
}

void xkb_hash_set_add_entry(
		struct xkb_hash_set *hash_set, uint32_t key, xkb_mod_mask_t mask) {
	assert(key < (sizeof(hash_set->set) / sizeof(hash_set->set[0])));
	// Strip out caps lock, mod 2, and any since those should be ignored.
	mask &= ~(XCB_MOD_MASK_LOCK | XCB_MOD_MASK_2 | XCB_MOD_MASK_ANY);

	struct hash_entry *entry = &hash_set->set[key];
	if (!entry->present) {
		entry->present = true;
		entry->mod_mask = mask;
	} else {
		while (entry->next != NULL) {
			entry = entry->next;
		}
		entry->next = calloc(1, sizeof(struct hash_entry));
		entry->next->present = true;
		entry->next->mod_mask = mask;
	}
}

bool xkb_hash_set_get_entry(
		struct xkb_hash_set *hash_set, uint32_t key, xkb_mod_mask_t mask) {
	assert(key < sizeof(hash_set->set) / sizeof(hash_set->set[0]));
	// Strip out caps lock, mod 2, and any since those should be ignored.
	mask &= ~(XCB_MOD_MASK_LOCK | XCB_MOD_MASK_2 | XCB_MOD_MASK_ANY);

	struct hash_entry *entry = &hash_set->set[key];
	if (entry->present) {
		while (entry != NULL) {
			if (entry->mod_mask == mask) {
				return true;
			}
			entry = entry->next;
		}
	}
	return false;
}
