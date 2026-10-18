# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake stateful compatibility fixture.
out: child
	@printf 'parent\n'
	@cp child out
child: trigger
	@printf 'child\n'
	@printf 'child\n' > child
	@python3 -c 'import os; os.utime("child", ns=(1600000000000000000, 1600000000000000000))'
