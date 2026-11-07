all: CAPTURED != printf '%b' '\0150\0145\0141\0144\0012\0040\0040'
all: CALLED := $(shell printf '%b' '\0150\0145\0141\0144\0012\0040\0040')
all: ; @$(info assign=[$(value CAPTURED)] func=[$(value CALLED)] status=[$(.SHELLSTATUS)])
