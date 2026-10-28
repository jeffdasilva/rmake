# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
$(shell touch -t 200001010000.00 out; touch -t 200101010000.00 newer; touch -t 199901010000.00 older)
out: newer newer older;@printf 'done\n'
