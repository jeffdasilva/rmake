BASE := $(shell /bin/sh -c 'exit 7')
chosen: VALUE != /bin/sh -c 'printf payload; exit 3'
chosen: CHECK := $(.SHELLSTATUS)
$(info reader=[$(.SHELLSTATUS)])
.PHONY: chosen other
all: chosen other
chosen: ; $(info chosen=[$(VALUE)/$(flavor VALUE)/$(CHECK)/$(.SHELLSTATUS)])@:
other: ; $(info other=[$(.SHELLSTATUS)])@:
