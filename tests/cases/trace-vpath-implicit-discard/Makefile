# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
$(shell mkdir -p results; touch -t 200001010000.00 results/x.out; touch -t 200101010000.00 x.in results/x.in)
VPATH=results
all: x.out
%.out: %.in;@printf 'done\n'
