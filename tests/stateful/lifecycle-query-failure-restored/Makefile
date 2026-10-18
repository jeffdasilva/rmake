# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake lifecycle fixture; see docs/forced-touch.md for behavioral references.
.DELETE_ON_ERROR:
out: source
	+@printf changed > out; touch -r stamp out; exit 1
verify:
	@test ! -e forbidden
	@test "$$(cat out)" = changed
	@printf 'verified\n'
