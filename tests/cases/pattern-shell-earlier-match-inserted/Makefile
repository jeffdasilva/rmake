special.%.job: VALUE != $(eval %.job: NEW := inserted)/usr/bin/printf '$(NEW)'
.PHONY: all special.one.job special.two.job
all: special.one.job special.two.job
special.one.job special.two.job: ; $(info $@=[$(VALUE)/$(NEW)])@:
