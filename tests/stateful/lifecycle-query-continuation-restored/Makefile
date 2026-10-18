# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake lifecycle fixture; see docs/forced-touch.md for behavioral references.
out: source
	+@printf changed > out; touch -r stamp out
	@printf forbidden > forbidden
verify:
	@test ! -e forbidden
	@test ! -e out
	@printf 'verified\n'
