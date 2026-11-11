
/home/asad/.local/share/aseprite/1.3.18.6/aseprite:     file format elf64-x86-64


Disassembly of section .text:

000000000050ae00 <_ZN3app6script12_GLOBAL__N_115App_get_bgColorEP9lua_State>:
  50ae00:	41 56                	push   %r14
  50ae02:	53                   	push   %rbx
  50ae03:	50                   	push   %rax
  50ae04:	48 89 fb             	mov    %rdi,%rbx
  50ae07:	e8 c4 79 cd ff       	call   1e27d0 <_ZN3app11Preferences8instanceEv>
  50ae0c:	49 89 c6             	mov    %rax,%r14
  50ae0f:	be 28 00 00 00       	mov    $0x28,%esi
  50ae14:	48 89 df             	mov    %rbx,%rdi
  50ae17:	ba 01 00 00 00       	mov    $0x1,%edx
  50ae1c:	e8 5f 63 03 00       	call   541180 <_Z17lua_newuserdatauvP9lua_Statemi>
  50ae21:	41 0f 10 86 20 48 00 	movups 0x4820(%r14),%xmm0
  50ae28:	00
  50ae29:	41 0f 10 8e 30 48 00 	movups 0x4830(%r14),%xmm1
  50ae30:	00
  50ae31:	0f 11 00             	movups %xmm0,(%rax)
  50ae34:	0f 11 48 10          	movups %xmm1,0x10(%rax)
  50ae38:	49 8b 8e 40 48 00 00 	mov    0x4840(%r14),%rcx
  50ae3f:	48 89 48 20          	mov    %rcx,0x20(%rax)
  50ae43:	e8 c8 a4 eb ff       	call   3c5310 <_ZN3app6script10get_mtnameINS_5ColorEEEPKcv>
  50ae48:	48 89 df             	mov    %rbx,%rdi
  50ae4b:	be d8 b9 f0 ff       	mov    $0xfff0b9d8,%esi
  50ae50:	48 89 c2             	mov    %rax,%rdx
  50ae53:	e8 b8 48 03 00       	call   53f710 <_Z12lua_getfieldP9lua_StateiPKc>
  50ae58:	48 89 df             	mov    %rbx,%rdi
  50ae5b:	be fe ff ff ff       	mov    $0xfffffffe,%esi
  50ae60:	e8 6b 56 03 00       	call   5404d0 <_Z16lua_setmetatableP9lua_Statei>
  50ae65:	b8 01 00 00 00       	mov    $0x1,%eax
  50ae6a:	48 83 c4 08          	add    $0x8,%rsp
  50ae6e:	5b                   	pop    %rbx
  50ae6f:	41 5e                	pop    %r14
  50ae71:	c3                   	ret
