all: CAPTURED != printf '%b' '\0150\0145\0141\0144\0012\0012\0000\0164\0141\0151\0154'
all: CALLED := $(shell printf '%b' '\0150\0145\0141\0144\0012\0012\0000\0164\0141\0151\0154')
all: ; @$(info assign=[$(value CAPTURED)] func=[$(value CALLED)] status=[$(.SHELLSTATUS)])
