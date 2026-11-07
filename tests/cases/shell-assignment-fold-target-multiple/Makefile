all: CAPTURED != printf '%b' '\0164\0145\0170\0164\0012\0012\0012'
all: CALLED := $(shell printf '%b' '\0164\0145\0170\0164\0012\0012\0012')
all: ; @$(info assign=[$(value CAPTURED)] func=[$(value CALLED)] status=[$(.SHELLSTATUS)])
