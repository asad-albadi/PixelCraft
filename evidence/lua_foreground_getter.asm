
aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000050ac90 <_ZN3app6script12_GLOBAL__N_115App_get_fgColorEP9lua_State>:
  50ac90:	41 56                	push   %r14
  50ac92:	53                   	push   %rbx
  50ac93:	50                   	push   %rax
  50ac94:	48 89 fb             	mov    %rdi,%rbx
  50ac97:	e8 34 7b cd ff       	call   1e27d0 <_ZN3app11Preferences8instanceEv>
  50ac9c:	49 89 c6             	mov    %rax,%r14
  50ac9f:	be 28 00 00 00       	mov    $0x28,%esi
  50aca4:	48 89 df             	mov    %rbx,%rdi
  50aca7:	ba 01 00 00 00       	mov    $0x1,%edx
  50acac:	e8 cf 64 03 00       	call   541180 <_Z17lua_newuserdatauvP9lua_Statemi>
  50acb1:	41 0f 10 86 30 47 00 	movups 0x4730(%r14),%xmm0
  50acb8:	00 
  50acb9:	41 0f 10 8e 40 47 00 	movups 0x4740(%r14),%xmm1
  50acc0:	00 
  50acc1:	0f 11 00             	movups %xmm0,(%rax)
  50acc4:	0f 11 48 10          	movups %xmm1,0x10(%rax)
  50acc8:	49 8b 8e 50 47 00 00 	mov    0x4750(%r14),%rcx
  50accf:	48 89 48 20          	mov    %rcx,0x20(%rax)
  50acd3:	e8 38 a6 eb ff       	call   3c5310 <_ZN3app6script10get_mtnameINS_5ColorEEEPKcv>
  50acd8:	48 89 df             	mov    %rbx,%rdi
  50acdb:	be d8 b9 f0 ff       	mov    $0xfff0b9d8,%esi
  50ace0:	48 89 c2             	mov    %rax,%rdx
  50ace3:	e8 28 4a 03 00       	call   53f710 <_Z12lua_getfieldP9lua_StateiPKc>
  50ace8:	48 89 df             	mov    %rbx,%rdi
  50aceb:	be fe ff ff ff       	mov    $0xfffffffe,%esi
  50acf0:	e8 db 57 03 00       	call   5404d0 <_Z16lua_setmetatableP9lua_Statei>
  50acf5:	b8 01 00 00 00       	mov    $0x1,%eax
  50acfa:	48 83 c4 08          	add    $0x8,%rsp
  50acfe:	5b                   	pop    %rbx
  50acff:	41 5e                	pop    %r14
  50ad01:	c3                   	ret
