# GNU binutils reference encodings; keep ordinary forms uncompressed.
.option norvc
.option norelax
.text
.global _start
_start:
sh1add a0, a1, a2
sh2add a0, a1, a2
sh3add a0, a1, a2
andn a0, a1, a2
orn a0, a1, a2
xnor a0, a1, a2
min a0, a1, a2
minu a0, a1, a2
max a0, a1, a2
maxu a0, a1, a2
rol a0, a1, a2
ror a0, a1, a2
bclr a0, a1, a2
bext a0, a1, a2
binv a0, a1, a2
bset a0, a1, a2
pack a0, a1, a2
packh a0, a1, a2
rori a0, a1, 31
bclri a0, a1, 31
bexti a0, a1, 31
binvi a0, a1, 31
bseti a0, a1, 31
clz a0, a1
ctz a0, a1
cpop a0, a1
sext.b a0, a1
sext.h a0, a1
zext.h a0, a1
orc.b a0, a1
rev8 a0, a1
brev8 a0, a1
zip a0, a1
unzip a0, a1
zext.b a0, a1
.option rvc
c.lbu a0, 3(a1)
c.lh a0, 2(a1)
c.lhu a0, 2(a1)
c.sb a0, 3(a1)
c.sh a0, 2(a1)
c.zext.b a0
c.sext.b a0
c.zext.h a0
c.sext.h a0
c.not a0
c.mul a0, a1
