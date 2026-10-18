# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake lifecycle fixture; see docs/forced-touch.md for behavioral references.
out: source
	@printf forbidden > forbidden
	+@true
verify:
	@test ! -e forbidden
	@test "$$(cat out)" = payload
	@printf 'verified\n'
