# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 1988-2023 Free Software Foundation, Inc.
# Observed and organized for rmake in 2026 by Jeff DaSilva.
# Linux GNU Make 4.4.1 compatibility data observed with the pinned oracle.
# Recipe and variable bytes are declarative compatibility data, not C source.
(%): %
	$(AR) $(ARFLAGS) $@ $<

%.out: %
	@rm -f $@ 
	 cp $< $@

%.c: %.w %.ch
	$(CTANGLE) $^ $@

%.tex: %.w %.ch
	$(CWEAVE) $^ $@

%:: %,v
	$(CHECKOUT,v)

%:: RCS/%,v
	$(CHECKOUT,v)

%:: RCS/%
	$(CHECKOUT,v)

%:: s.%
	$(GET) $(GFLAGS) $(SCCS_OUTPUT_OPTION) $<

%:: SCCS/s.%
	$(GET) $(GFLAGS) $(SCCS_OUTPUT_OPTION) $<
