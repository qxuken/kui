/* What every C example shares: the ABI check each one starts with, and
 * the assertion helper the tools count failures with. Header-only, so the
 * build scripts compile one file per program and nothing else.
 *
 * KUI_ABI_VERSION is the one this binary was compiled against; the
 * library says its own. The example reads arrays out of the library —
 * kui_poll_event's KuiEvent, KuiDrawData's quads, the access nodes — and
 * reserves sizeof(...) as this header declares it, so a newer library that
 * appended a field would write past the end of that. The `size` in
 * KUI_EVENT_INIT stops exactly that one, but only for [out] structs; the
 * arrays have no in-band guard, and this check is what stands in for one.
 * Equality, not >=, for the reason kui.h gives. */
#ifndef KUI_EXAMPLES_COMMON_H
#define KUI_EXAMPLES_COMMON_H

#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include "kui.h"

static inline int abi_ok(void) {
    uint32_t lib = kui_abi_version();
    if (lib == KUI_ABI_VERSION) return 1;
    fprintf(stderr,
            "FAIL: libkui_ffi implements ABI %u, this binary was built "
            "against ABI %u.\n"
            "      Rebuild against the matching kui.h, or link the matching "
            "library.\n",
            lib, (unsigned)KUI_ABI_VERSION);
    return 0;
}

/* One assertion: says what failed and counts it, so a program runs its
 * whole list and reports every miss rather than the first. The counter is
 * marked unused for gcc and clang, whose -Wall names a static a program
 * never reads; cl has no such warning and no such syntax, and rejected the
 * attribute outright the first time the C round ran on Windows. */
#if defined(__GNUC__) || defined(__clang__)
#define KUI_EXAMPLES_UNUSED __attribute__((unused))
#else
#define KUI_EXAMPLES_UNUSED
#endif
static int fails KUI_EXAMPLES_UNUSED;

static inline void check(bool ok, const char *what) {
    if (!ok) {
        fprintf(stderr, "FAIL: %s\n", what);
        fails++;
    }
}

/* KuiStr is not NUL-terminated and memmem is not standard C. */
static inline bool has(KuiStr s, const char *needle) {
    size_t n = strlen(needle);
    if (n > s.len) return false;
    for (size_t i = 0; i + n <= s.len; i++) {
        if (memcmp(s.ptr + i, needle, n) == 0) return true;
    }
    return false;
}

#endif
