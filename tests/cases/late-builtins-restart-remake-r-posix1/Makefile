# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
.POSIX:
$(info read-restart=[$(MAKE_RESTARTS)];CC=[$(CC)]/$(origin CC)/$(flavor CC),CXX=[$(CXX)]/$(origin CXX),lib=[$(.LIBPATTERNS)]/$(origin .LIBPATTERNS),suffix=[$(words $(SUFFIXES))]/$(origin SUFFIXES),shell=$(origin SHELL)/$(origin .SHELLFLAGS),make=$(origin MAKE_COMMAND),gnu=$(origin GNUMAKEFLAGS),metadata-count=$(words $(filter CC CXX .LIBPATTERNS .FEATURES .SHELLFLAGS GNUMAKEFLAGS MAKEFILE_LIST,$(.VARIABLES))))
-include flags.mk
flags.mk:;@$(eval MAKEFLAGS += -r)printf "# generated\n" >$@
probe.o:;@$(info recipe=CC=[$(CC)]/$(origin CC)/$(flavor CC),CXX=[$(CXX)]/$(origin CXX),lib=[$(.LIBPATTERNS)]/$(origin .LIBPATTERNS),suffix=[$(words $(SUFFIXES))]/$(origin SUFFIXES),shell=$(origin SHELL)/$(origin .SHELLFLAGS),make=$(origin MAKE_COMMAND),gnu=$(origin GNUMAKEFLAGS),metadata-count=$(words $(filter CC CXX .LIBPATTERNS .FEATURES .SHELLFLAGS GNUMAKEFLAGS MAKEFILE_LIST,$(.VARIABLES)));stem=[$*]):
