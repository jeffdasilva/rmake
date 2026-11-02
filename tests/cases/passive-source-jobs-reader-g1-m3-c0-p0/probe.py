import fcntl, os, sys
flags, auth = sys.argv[1:]
tokens = b''
if auth:
    value = auth.split('=', 1)[1]
    owned = value.startswith('fifo:')
    if owned:
        read_fd = os.open(value[5:], os.O_RDONLY | os.O_NONBLOCK)
        write_fd = os.open(value[5:], os.O_WRONLY | os.O_NONBLOCK)
    else:
        read_fd, write_fd = map(int, value.split(','))
    saved = fcntl.fcntl(read_fd, fcntl.F_GETFL)
    try:
        fcntl.fcntl(read_fd, fcntl.F_SETFL, saved | os.O_NONBLOCK)
        try:
            tokens = os.read(read_fd, 4096)
        except BlockingIOError:
            pass
    finally:
        fcntl.fcntl(read_fd, fcntl.F_SETFL, saved)
        if tokens:
            assert os.write(write_fd, tokens) == len(tokens)
        if owned:
            os.close(read_fd)
            os.close(write_fd)
print('FLAGS=[' + flags + '] AUTH=[' + ('yes' if auth else 'no') + '] TOKENS=[' + str(len(tokens)) + ']')
