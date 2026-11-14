# Output synchronization at low descriptor limits

The six cases run `-j -Otarget` with separate or merged output. They check
successful recipes at limits 6 and 5, a roomy control, physical identity of
the two recipe streams, and an ignored exec error followed by a real recipe.
The original stream witness requires regular files and distinguishes a
shared spool from separate spools.

Run `tools/resource_fd_spool.py --candidate PATH --report PATH`, or use
`--oracle-only`. The focused ID is `spool-fd6-separate-identity`. The gate
checks exact streams, status, full fixture filesystem, and process
containment with two independent runs of the verified GNU oracle.

The `runtime-output-owned-fifo-launch-pressure-*` fixtures exercise target
output sync with an owned FIFO and a live wake pipe. If the standard launch
runs out of descriptors, an inherited-stdin recipe can borrow the existing
spool streams for the ordered spawn. Both split and merged output retain
their usual buffering and order.

The `runtime-output-owned-fifo-bounded-wait-*` cases cover the same retry
at descriptor limits where fatal signal delivery uses the existing bounded
wait. Both target output paths keep their original spool lifetime.
