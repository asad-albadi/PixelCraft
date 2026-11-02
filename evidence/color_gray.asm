
aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

00000000003c68f0 <_ZN3app6script12_GLOBAL__N_114Color_get_grayEP9lua_State>:
  3c68f0:	53                   	push   %rbx
  3c68f1:	48 89 fb             	mov    %rdi,%rbx
  3c68f4:	48 8d 15 51 92 b7 00 	lea    0xb79251(%rip),%rdx        # f3fb4c <_ZL16gUnaryActiveEdge+0xc3e8>
  3c68fb:	be 01 00 00 00       	mov    $0x1,%esi
  3c6900:	e8 7b 8a 16 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3c6905:	48 89 c7             	mov    %rax,%rdi
  3c6908:	e8 a3 59 da ff       	call   16c2b0 <_ZNK3app5Color7getGrayEv>
  3c690d:	f2 0f 2a c0          	cvtsi2sd %eax,%xmm0
  3c6911:	48 89 df             	mov    %rbx,%rdi
  3c6914:	e8 d7 88 17 00       	call   53f1f0 <_Z14lua_pushnumberP9lua_Stated>
  3c6919:	b8 01 00 00 00       	mov    $0x1,%eax
  3c691e:	5b                   	pop    %rbx
  3c691f:	c3                   	ret

aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

00000000003c6a20 <_ZN3app6script12_GLOBAL__N_119Color_get_grayPixelEP9lua_State>:
  3c6a20:	53                   	push   %rbx
  3c6a21:	48 83 ec 20          	sub    $0x20,%rsp
  3c6a25:	48 89 fb             	mov    %rdi,%rbx
  3c6a28:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c6a2f:	00 00
  3c6a31:	48 89 44 24 18       	mov    %rax,0x18(%rsp)
  3c6a36:	48 8d 15 0f 91 b7 00 	lea    0xb7910f(%rip),%rdx        # f3fb4c <_ZL16gUnaryActiveEdge+0xc3e8>
  3c6a3d:	be 01 00 00 00       	mov    $0x1,%esi
  3c6a42:	e8 39 89 16 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3c6a47:	48 b9 01 00 00 00 01 	movabs $0x100000001,%rcx
  3c6a4e:	00 00 00
  3c6a51:	48 89 4c 24 0c       	mov    %rcx,0xc(%rsp)
  3c6a56:	c7 44 24 14 00 00 00 	movl   $0x0,0x14(%rsp)
  3c6a5d:	00
  3c6a5e:	48 8d 74 24 0c       	lea    0xc(%rsp),%rsi
  3c6a63:	48 89 c7             	mov    %rax,%rdi
  3c6a66:	e8 25 72 da ff       	call   16dc90 <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE>
  3c6a6b:	89 c6                	mov    %eax,%esi
  3c6a6d:	48 89 df             	mov    %rbx,%rdi
  3c6a70:	e8 9b 87 17 00       	call   53f210 <_Z15lua_pushintegerP9lua_Statex>
  3c6a75:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c6a7c:	00 00
  3c6a7e:	48 3b 44 24 18       	cmp    0x18(%rsp),%rax
  3c6a83:	75 0b                	jne    3c6a90 <_ZN3app6script12_GLOBAL__N_119Color_get_grayPixelEP9lua_State+0x70>
  3c6a85:	b8 01 00 00 00       	mov    $0x1,%eax
  3c6a8a:	48 83 c4 20          	add    $0x20,%rsp
  3c6a8e:	5b                   	pop    %rbx
  3c6a8f:	c3                   	ret
  3c6a90:	e8 4b fb d7 ff       	call   1465e0 <__stack_chk_fail@plt>

aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000016dc90 <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE>:
  16dc90:	55                   	push   %rbp
  16dc91:	41 57                	push   %r15
  16dc93:	41 56                	push   %r14
  16dc95:	41 54                	push   %r12
  16dc97:	53                   	push   %rbx
  16dc98:	8b 0f                	mov    (%rdi),%ecx
  16dc9a:	85 c9                	test   %ecx,%ecx
  16dc9c:	74 66                	je     16dd04 <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE+0x74>
  16dc9e:	8b 56 04             	mov    0x4(%rsi),%edx
  16dca1:	b8 ff ff ff ff       	mov    $0xffffffff,%eax
  16dca6:	48 83 fa 04          	cmp    $0x4,%rdx
  16dcaa:	0f 87 89 00 00 00    	ja     16dd39 <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE+0xa9>
  16dcb0:	4c 8d 05 ed 3e e1 00 	lea    0xe13eed(%rip),%r8        # f81ba4 <_ZTSZN3app23initialize_color_spacesERNS_11PreferencesEE3$_0+0x60>
  16dcb7:	49 63 14 90          	movslq (%r8,%rdx,4),%rdx
  16dcbb:	4c 01 c2             	add    %r8,%rdx
  16dcbe:	ff e2                	jmp    *%rdx
  16dcc0:	48 89 fb             	mov    %rdi,%rbx
  16dcc3:	e8 78 d1 ff ff       	call   16ae40 <_ZNK3app5Color6getRedEv>
  16dcc8:	89 c5                	mov    %eax,%ebp
  16dcca:	48 89 df             	mov    %rbx,%rdi
  16dccd:	e8 ce d2 ff ff       	call   16afa0 <_ZNK3app5Color8getGreenEv>
  16dcd2:	41 89 c6             	mov    %eax,%r14d
  16dcd5:	48 89 df             	mov    %rbx,%rdi
  16dcd8:	e8 43 d4 ff ff       	call   16b120 <_ZNK3app5Color7getBlueEv>
  16dcdd:	41 89 c7             	mov    %eax,%r15d
  16dce0:	48 89 df             	mov    %rbx,%rdi
  16dce3:	e8 b8 d5 ff ff       	call   16b2a0 <_ZNK3app5Color8getAlphaEv>
  16dce8:	40 0f b6 cd          	movzbl %bpl,%ecx
  16dcec:	41 c1 e6 08          	shl    $0x8,%r14d
  16dcf0:	41 0f b7 d6          	movzwl %r14w,%edx
  16dcf4:	09 ca                	or     %ecx,%edx
  16dcf6:	41 0f b6 cf          	movzbl %r15b,%ecx
  16dcfa:	c1 e1 10             	shl    $0x10,%ecx
  16dcfd:	09 d1                	or     %edx,%ecx
  16dcff:	c1 e0 18             	shl    $0x18,%eax
  16dd02:	eb 33                	jmp    16dd37 <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE+0xa7>
  16dd04:	8b 46 08             	mov    0x8(%rsi),%eax
  16dd07:	eb 30                	jmp    16dd39 <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE+0xa9>
  16dd09:	83 f9 05             	cmp    $0x5,%ecx
  16dd0c:	75 41                	jne    16dd4f <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE+0xbf>
  16dd0e:	5b                   	pop    %rbx
  16dd0f:	41 5c                	pop    %r12
  16dd11:	41 5e                	pop    %r14
  16dd13:	41 5f                	pop    %r15
  16dd15:	5d                   	pop    %rbp
  16dd16:	e9 95 ee ff ff       	jmp    16cbb0 <_ZNK3app5Color8getIndexEv>
  16dd1b:	48 89 fb             	mov    %rdi,%rbx
  16dd1e:	e8 8d e5 ff ff       	call   16c2b0 <_ZNK3app5Color7getGrayEv>
  16dd23:	89 c5                	mov    %eax,%ebp
  16dd25:	48 89 df             	mov    %rbx,%rdi
  16dd28:	e8 73 d5 ff ff       	call   16b2a0 <_ZNK3app5Color8getAlphaEv>
  16dd2d:	40 0f b6 cd          	movzbl %bpl,%ecx
  16dd31:	c1 e0 08             	shl    $0x8,%eax
  16dd34:	0f b7 c0             	movzwl %ax,%eax
  16dd37:	09 c8                	or     %ecx,%eax
  16dd39:	5b                   	pop    %rbx
  16dd3a:	41 5c                	pop    %r12
  16dd3c:	41 5e                	pop    %r14
  16dd3e:	41 5f                	pop    %r15
  16dd40:	5d                   	pop    %rbp
  16dd41:	c3                   	ret
  16dd42:	5b                   	pop    %rbx
  16dd43:	41 5c                	pop    %r12
  16dd45:	41 5e                	pop    %r14
  16dd47:	41 5f                	pop    %r15
  16dd49:	5d                   	pop    %rbp
  16dd4a:	e9 61 f0 ff ff       	jmp    16cdb0 <_ZNK3app5Color7getTileEv>
  16dd4f:	49 89 ff             	mov    %rdi,%r15
  16dd52:	49 89 f4             	mov    %rsi,%r12
  16dd55:	e8 e6 d0 ff ff       	call   16ae40 <_ZNK3app5Color6getRedEv>
  16dd5a:	89 c3                	mov    %eax,%ebx
  16dd5c:	4c 89 ff             	mov    %r15,%rdi
  16dd5f:	e8 3c d2 ff ff       	call   16afa0 <_ZNK3app5Color8getGreenEv>
  16dd64:	89 c5                	mov    %eax,%ebp
  16dd66:	4c 89 ff             	mov    %r15,%rdi
  16dd69:	e8 b2 d3 ff ff       	call   16b120 <_ZNK3app5Color7getBlueEv>
  16dd6e:	41 89 c6             	mov    %eax,%r14d
  16dd71:	4c 89 ff             	mov    %r15,%rdi
  16dd74:	e8 27 d5 ff ff       	call   16b2a0 <_ZNK3app5Color8getAlphaEv>
  16dd79:	4c 89 e1             	mov    %r12,%rcx
  16dd7c:	41 89 c7             	mov    %eax,%r15d
  16dd7f:	41 bc ff ff ff ff    	mov    $0xffffffff,%r12d
  16dd85:	83 39 01             	cmpl   $0x1,(%rcx)
  16dd88:	75 04                	jne    16dd8e <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE+0xfe>
  16dd8a:	44 8b 61 08          	mov    0x8(%rcx),%r12d
  16dd8e:	e8 b5 47 07 00       	call   1e2548 <_ZN3app19get_current_paletteEv>
  16dd93:	48 89 c7             	mov    %rax,%rdi
  16dd96:	89 de                	mov    %ebx,%esi
  16dd98:	89 ea                	mov    %ebp,%edx
  16dd9a:	44 89 f1             	mov    %r14d,%ecx
  16dd9d:	45 89 f8             	mov    %r15d,%r8d
  16dda0:	45 89 e1             	mov    %r12d,%r9d
  16dda3:	e8 88 d6 49 00       	call   60b430 <_ZNK3doc7Palette14findExactMatchEiiiii>
  16dda8:	85 c0                	test   %eax,%eax
  16ddaa:	79 8d                	jns    16dd39 <_ZN3app11color_utils21color_for_target_maskERKNS_5ColorERKNS_11ColorTargetE+0xa9>
  16ddac:	e8 97 47 07 00       	call   1e2548 <_ZN3app19get_current_paletteEv>
  16ddb1:	48 89 c7             	mov    %rax,%rdi
  16ddb4:	89 de                	mov    %ebx,%esi
  16ddb6:	89 ea                	mov    %ebp,%edx
  16ddb8:	44 89 f1             	mov    %r14d,%ecx
  16ddbb:	45 89 f8             	mov    %r15d,%r8d
  16ddbe:	45 89 e1             	mov    %r12d,%r9d
  16ddc1:	5b                   	pop    %rbx
  16ddc2:	41 5c                	pop    %r12
  16ddc4:	41 5e                	pop    %r14
  16ddc6:	41 5f                	pop    %r15
  16ddc8:	5d                   	pop    %rbp
  16ddc9:	e9 b2 d9 49 00       	jmp    60b780 <_ZNK3doc7Palette11findBestfitEiiiii>
  16ddce:	66 90                	xchg   %ax,%ax

000000000016ddd0 <_ZN3app15OpenFileCommandC1Ev>:
  16ddd0:	53                   	push   %rbx
  16ddd1:	48 89 fb             	mov    %rdi,%rbx
  16ddd4:	48 8d 35 1e 75 dc 00 	lea    0xdc751e(%rip),%rsi        # f352f9 <_ZL16gUnaryActiveEdge+0x1b95>
  16dddb:	e8 d0 26 00 00       	call   1704b0 <_ZN3app7CommandC1EPKc>
  16dde0:	48 8d 05 c9 e0 b2 01 	lea    0x1b2e0c9(%rip),%rax        # 1c9beb0 <_ZTVN3app15OpenFileCommandE+0x10>
  16dde7:	48 89 03             	mov    %rax,(%rbx)
  16ddea:	48 8d 43 38          	lea    0x38(%rbx),%rax
  16ddee:	48 89 43 28          	mov    %rax,0x28(%rbx)
  16ddf2:	c6 43 38 00          	movb   $0x0,0x38(%rbx)
  16ddf6:	48 c7 43 30 00 00 00 	movq   $0x0,0x30(%rbx)
  16ddfd:	00
  16ddfe:	48 8d 43 58          	lea    0x58(%rbx),%rax
  16de02:	48 89 43 48          	mov    %rax,0x48(%rbx)
  16de06:	c6 43 58 00          	movb   $0x0,0x58(%rbx)
  16de0a:	48 c7 43 50 00 00 00 	movq   $0x0,0x50(%rbx)
  16de11:	00
  16de12:	66 c7 43 68 01 00    	movw   $0x1,0x68(%rbx)
  16de18:	c6 43 6a 00          	movb   $0x0,0x6a(%rbx)
  16de1c:	0f 57 c0             	xorps  %xmm0,%xmm0
  16de1f:	0f 11 43 70          	movups %xmm0,0x70(%rbx)
  16de23:	0f 11 43 7c          	movups %xmm0,0x7c(%rbx)
  16de27:	5b                   	pop    %rbx
  16de28:	c3                   	ret
  16de29:	90                   	nop
  16de2a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

000000000016de30 <_ZN3app15OpenFileCommand12onLoadParamsERKNS_6ParamsE>:
  16de30:	55                   	push   %rbp
  16de31:	41 57                	push   %r15
  16de33:	41 56                	push   %r14
  16de35:	53                   	push   %rbx
  16de36:	48 83 ec 28          	sub    $0x28,%rsp
  16de3a:	49 89 f6             	mov    %rsi,%r14
  16de3d:	48 89 fb             	mov    %rdi,%rbx
  16de40:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  16de47:	00 00
  16de49:	48 89 44 24 20       	mov    %rax,0x20(%rsp)
  16de4e:	48 8d 15 5a 5d e0 00 	lea    0xe05d5a(%rip),%rdx        # f73baf <KW_UTF_16LE+0x2682>
  16de55:	48 89 e7             	mov    %rsp,%rdi
  16de58:	e8 53 17 00 00       	call   16f5b0 <_ZNK3app6Params3getB5cxx11EPKc>
  16de5d:	48                   	rex.W
  16de5e:	8b                   	.byte 0x8b
  16de5f:	7b                   	.byte 0x7b

aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000016c2b0 <_ZNK3app5Color7getGrayEv>:
  16c2b0:	55                   	push   %rbp
  16c2b1:	41 56                	push   %r14
  16c2b3:	53                   	push   %rbx
  16c2b4:	48 83 ec 30          	sub    $0x30,%rsp
  16c2b8:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  16c2bf:	00 00
  16c2c1:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  16c2c6:	8b 07                	mov    (%rdi),%eax
  16c2c8:	48 83 f8 05          	cmp    $0x5,%rax
  16c2cc:	0f 87 fb 00 00 00    	ja     16c3cd <_ZNK3app5Color7getGrayEv+0x11d>
  16c2d2:	48 8d 0d 2f 57 e1 00 	lea    0xe1572f(%rip),%rcx        # f81a08 <_ZTSN3app18PreviewCliDelegateE+0x128>
  16c2d9:	48 63 14 81          	movslq (%rcx,%rax,4),%rdx
  16c2dd:	48 01 ca             	add    %rcx,%rdx
  16c2e0:	ff e2                	jmp    *%rdx
  16c2e2:	8b 47 10             	mov    0x10(%rdi),%eax
  16c2e5:	48 8b 4f 08          	mov    0x8(%rdi),%rcx
  16c2e9:	48 89 0c 24          	mov    %rcx,(%rsp)
  16c2ed:	e9 9f 00 00 00       	jmp    16c391 <_ZNK3app5Color7getGrayEv+0xe1>
  16c2f2:	8b 47 08             	mov    0x8(%rdi),%eax
  16c2f5:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  16c2fc:	00 00
  16c2fe:	48 3b 4c 24 28       	cmp    0x28(%rsp),%rcx
  16c303:	0f 84 bb 00 00 00    	je     16c3c4 <_ZNK3app5Color7getGrayEv+0x114>
  16c309:	e9 d4 00 00 00       	jmp    16c3e2 <_ZNK3app5Color7getGrayEv+0x132>
  16c30e:	48 89 fb             	mov    %rdi,%rbx
  16c311:	e8 2a eb ff ff       	call   16ae40 <_ZNK3app5Color6getRedEv>
  16c316:	89 c5                	mov    %eax,%ebp
  16c318:	48 89 df             	mov    %rbx,%rdi
  16c31b:	e8 80 ec ff ff       	call   16afa0 <_ZNK3app5Color8getGreenEv>
  16c320:	41 89 c6             	mov    %eax,%r14d
  16c323:	48 89 df             	mov    %rbx,%rdi
  16c326:	e8 f5 ed ff ff       	call   16b120 <_ZNK3app5Color7getBlueEv>
  16c32b:	89 2c 24             	mov    %ebp,(%rsp)
  16c32e:	44 89 74 24 04       	mov    %r14d,0x4(%rsp)
  16c333:	eb 5c                	jmp    16c391 <_ZNK3app5Color7getGrayEv+0xe1>
  16c335:	f2 0f 10 47 18       	movsd  0x18(%rdi),%xmm0
  16c33a:	eb 6c                	jmp    16c3a8 <_ZNK3app5Color7getGrayEv+0xf8>
  16c33c:	48 63 5f 08          	movslq 0x8(%rdi),%rbx
  16c340:	31 c0                	xor    %eax,%eax
  16c342:	48 85 db             	test   %rbx,%rbx
  16c345:	78 6d                	js     16c3b4 <_ZNK3app5Color7getGrayEv+0x104>
  16c347:	e8 fc 61 07 00       	call   1e2548 <_ZN3app19get_current_paletteEv>
  16c34c:	48 89 c1             	mov    %rax,%rcx
  16c34f:	31 c0                	xor    %eax,%eax
  16c351:	48 8b 51 28          	mov    0x28(%rcx),%rdx
  16c355:	48 2b 51 20          	sub    0x20(%rcx),%rdx
  16c359:	48 c1 ea 02          	shr    $0x2,%rdx
  16c35d:	39 d3                	cmp    %edx,%ebx
  16c35f:	7d 53                	jge    16c3b4 <_ZNK3app5Color7getGrayEv+0x104>
  16c361:	e8 e2 61 07 00       	call   1e2548 <_ZN3app19get_current_paletteEv>
  16c366:	48 8b 48 20          	mov    0x20(%rax),%rcx
  16c36a:	48 8b 50 28          	mov    0x28(%rax),%rdx
  16c36e:	48 29 ca             	sub    %rcx,%rdx
  16c371:	48 c1 ea 02          	shr    $0x2,%rdx
  16c375:	31 c0                	xor    %eax,%eax
  16c377:	39 d3                	cmp    %edx,%ebx
  16c379:	7d 03                	jge    16c37e <_ZNK3app5Color7getGrayEv+0xce>
  16c37b:	8b 04 99             	mov    (%rcx,%rbx,4),%eax
  16c37e:	0f b6 c8             	movzbl %al,%ecx
  16c381:	0f b6 d4             	movzbl %ah,%edx
  16c384:	c1 e8 10             	shr    $0x10,%eax
  16c387:	0f b6 c0             	movzbl %al,%eax
  16c38a:	89 0c 24             	mov    %ecx,(%rsp)
  16c38d:	89 54 24 04          	mov    %edx,0x4(%rsp)
  16c391:	89 44 24 08          	mov    %eax,0x8(%rsp)
  16c395:	48 8d 7c 24 10       	lea    0x10(%rsp),%rdi
  16c39a:	48 89 e6             	mov    %rsp,%rsi
  16c39d:	e8 0e 50 52 00       	call   6913b0 <_ZN3gfx3HslC1ERKNS_3RgbE>
  16c3a2:	f2 0f 10 44 24 20    	movsd  0x20(%rsp),%xmm0
  16c3a8:	f2 0f 59 05 48 f0 e0 	mulsd  0xe0f048(%rip),%xmm0        # f7b3f8 <_ZTSN2ui13MenuSeparatorE+0x2b>
  16c3af:	00
  16c3b0:	f2 0f 2c c0          	cvttsd2si %xmm0,%eax
  16c3b4:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  16c3bb:	00 00
  16c3bd:	48 3b 4c 24 28       	cmp    0x28(%rsp),%rcx
  16c3c2:	75 1e                	jne    16c3e2 <_ZNK3app5Color7getGrayEv+0x132>
  16c3c4:	48 83 c4 30          	add    $0x30,%rsp
  16c3c8:	5b                   	pop    %rbx
  16c3c9:	41 5e                	pop    %r14
  16c3cb:	5d                   	pop    %rbp
  16c3cc:	c3                   	ret
  16c3cd:	b8 ff ff ff ff       	mov    $0xffffffff,%eax
  16c3d2:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  16c3d9:	00 00
  16c3db:	48 3b 4c 24 28       	cmp    0x28(%rsp),%rcx
  16c3e0:	74 e2                	je     16c3c4 <_ZNK3app5Color7getGrayEv+0x114>
  16c3e2:	e8 f9 a1 fd ff       	call   1465e0 <__stack_chk_fail@plt>

aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

00000000006913b0 <_ZN3gfx3HslC1ERKNS_3RgbE>:
  6913b0:	41 57                	push   %r15
  6913b2:	41 56                	push   %r14
  6913b4:	53                   	push   %rbx
  6913b5:	48 83 ec 20          	sub    $0x20,%rsp
  6913b9:	49 89 f7             	mov    %rsi,%r15
  6913bc:	48 89 fb             	mov    %rdi,%rbx
  6913bf:	48 89 f7             	mov    %rsi,%rdi
  6913c2:	e8 69 06 00 00       	call   691a30 <_ZNK3gfx3Rgb12maxComponentEv>
  6913c7:	41 89 c6             	mov    %eax,%r14d
  6913ca:	4c 89 ff             	mov    %r15,%rdi
  6913cd:	e8 7e 06 00 00       	call   691a50 <_ZNK3gfx3Rgb12minComponentEv>
  6913d2:	42 8d 0c 30          	lea    (%rax,%r14,1),%ecx
  6913d6:	66 0f 6e d1          	movd   %ecx,%xmm2
  6913da:	44 89 f1             	mov    %r14d,%ecx
  6913dd:	66 0f 57 c0          	xorpd  %xmm0,%xmm0
  6913e1:	66 0f 57 c9          	xorpd  %xmm1,%xmm1
  6913e5:	29 c1                	sub    %eax,%ecx
  6913e7:	66 0f 6e d9          	movd   %ecx,%xmm3
  6913eb:	66 0f 62 da          	punpckldq %xmm2,%xmm3
  6913ef:	f3 0f e6 e3          	cvtdq2pd %xmm3,%xmm4
  6913f3:	66 0f 5e 25 05 ae 8e 	divpd  0x8eae05(%rip),%xmm4        # f7c200 <kAverageBytesPerMB+0x4c8>
  6913fa:	00
  6913fb:	66 0f 28 dc          	movapd %xmm4,%xmm3
  6913ff:	66 0f 15 dc          	unpckhpd %xmm4,%xmm3
  691403:	f2 0f 59 1d 8d a0 8e 	mulsd  0x8ea08d(%rip),%xmm3        # f7b498 <_ZTSN2ui13MenuSeparatorE+0xcb>
  69140a:	00
  69140b:	0f 84 03 01 00 00    	je     691514 <_ZN3gfx3HslC1ERKNS_3RgbE+0x164>
  691411:	f3 41 0f 7e 4f 04    	movq   0x4(%r15),%xmm1
  691417:	f3 0f e6 c1          	cvtdq2pd %xmm1,%xmm0
  69141b:	41 8b 07             	mov    (%r15),%eax
  69141e:	66 0f 5e 05 da ad 8e 	divpd  0x8eadda(%rip),%xmm0        # f7c200 <kAverageBytesPerMB+0x4c8>
  691425:	00
  691426:	41 39 c6             	cmp    %eax,%r14d
  691429:	75 5d                	jne    691488 <_ZN3gfx3HslC1ERKNS_3RgbE+0xd8>
  69142b:	66 0f 28 c8          	movapd %xmm0,%xmm1
  69142f:	66 0f 15 c8          	unpckhpd %xmm0,%xmm1
  691433:	f2 0f 5c c1          	subsd  %xmm1,%xmm0
  691437:	f2 0f 5e c4          	divsd  %xmm4,%xmm0
  69143b:	66 0f 57 c9          	xorpd  %xmm1,%xmm1
  69143f:	66 0f 2e c8          	ucomisd %xmm0,%xmm1
  691443:	66 0f 29 5c 24 10    	movapd %xmm3,0x10(%rsp)
  691449:	66 0f 29 24 24       	movapd %xmm4,(%rsp)
  69144e:	76 1a                	jbe    69146a <_ZN3gfx3HslC1ERKNS_3RgbE+0xba>
  691450:	f2 0f 10 15 58 a0 8e 	movsd  0x8ea058(%rip),%xmm2        # f7b4b0 <_ZTSN2ui13MenuSeparatorE+0xe3>
  691457:	00
  691458:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  69145f:	00
  691460:	f2 0f 58 c2          	addsd  %xmm2,%xmm0
  691464:	66 0f 2e c8          	ucomisd %xmm0,%xmm1
  691468:	77 f6                	ja     691460 <_ZN3gfx3HslC1ERKNS_3RgbE+0xb0>
  69146a:	f2 0f 10 0d 3e a0 8e 	movsd  0x8ea03e(%rip),%xmm1        # f7b4b0 <_ZTSN2ui13MenuSeparatorE+0xe3>
  691471:	00
  691472:	e8 c1 75 ab ff       	call   148a38 <fmod@plt>
  691477:	66 0f 28 c8          	movapd %xmm0,%xmm1
  69147b:	66 0f 28 5c 24 10    	movapd 0x10(%rsp),%xmm3
  691481:	66 0f 28 24 24       	movapd (%rsp),%xmm4
  691486:	eb 58                	jmp    6914e0 <_ZN3gfx3HslC1ERKNS_3RgbE+0x130>
  691488:	0f 57 d2             	xorps  %xmm2,%xmm2
  69148b:	f2 0f 2a d0          	cvtsi2sd %eax,%xmm2
  69148f:	f2 0f 5e 15 61 9f 8e 	divsd  0x8e9f61(%rip),%xmm2        # f7b3f8 <_ZTSN2ui13MenuSeparatorE+0x2b>
  691496:	00
  691497:	66 0f 7e c8          	movd   %xmm1,%eax
  69149b:	41 39 c6             	cmp    %eax,%r14d
  69149e:	75 1a                	jne    6914ba <_ZN3gfx3HslC1ERKNS_3RgbE+0x10a>
  6914a0:	66 0f 15 c0          	unpckhpd %xmm0,%xmm0
  6914a4:	f2 0f 5c c2          	subsd  %xmm2,%xmm0
  6914a8:	f2 0f 5e c4          	divsd  %xmm4,%xmm0
  6914ac:	f2 0f 58 05 6c a0 8e 	addsd  0x8ea06c(%rip),%xmm0        # f7b520 <_ZTSN2ui13MenuSeparatorE+0x153>
  6914b3:	00
  6914b4:	66 0f 28 c8          	movapd %xmm0,%xmm1
  6914b8:	eb 26                	jmp    6914e0 <_ZN3gfx3HslC1ERKNS_3RgbE+0x130>
  6914ba:	66 0f 70 c9 55       	pshufd $0x55,%xmm1,%xmm1
  6914bf:	66 0f 7e c8          	movd   %xmm1,%eax
  6914c3:	66 0f ef c9          	pxor   %xmm1,%xmm1
  6914c7:	41 39 c6             	cmp    %eax,%r14d
  6914ca:	75 14                	jne    6914e0 <_ZN3gfx3HslC1ERKNS_3RgbE+0x130>
  6914cc:	f2 0f 5c d0          	subsd  %xmm0,%xmm2
  6914d0:	f2 0f 5e d4          	divsd  %xmm4,%xmm2
  6914d4:	f2 0f 58 15 04 a1 8e 	addsd  0x8ea104(%rip),%xmm2        # f7b5e0 <_ZTSN2ui13MenuSeparatorE+0x213>
  6914db:	00
  6914dc:	66 0f 28 ca          	movapd %xmm2,%xmm1
  6914e0:	f2 0f 59 0d 38 a1 8e 	mulsd  0x8ea138(%rip),%xmm1        # f7b620 <_ZTSN2ui13MenuSeparatorE+0x253>
  6914e7:	00
  6914e8:	66 0f 28 c3          	movapd %xmm3,%xmm0
  6914ec:	f2 0f 58 c3          	addsd  %xmm3,%xmm0
  6914f0:	f2 0f 58 05 18 9f 8e 	addsd  0x8e9f18(%rip),%xmm0        # f7b410 <_ZTSN2ui13MenuSeparatorE+0x43>
  6914f7:	00
  6914f8:	66 0f 54 05 70 aa 8e 	andpd  0x8eaa70(%rip),%xmm0        # f7bf70 <kAverageBytesPerMB+0x238>
  6914ff:	00
  691500:	f2 0f 10 15 e0 9e 8e 	movsd  0x8e9ee0(%rip),%xmm2        # f7b3e8 <_ZTSN2ui13MenuSeparatorE+0x1b>
  691507:	00
  691508:	f2 0f 5c d0          	subsd  %xmm0,%xmm2
  69150c:	f2 0f 5e e2          	divsd  %xmm2,%xmm4
  691510:	66 0f 28 c4          	movapd %xmm4,%xmm0
  691514:	f2 0f 11 0b          	movsd  %xmm1,(%rbx)
  691518:	f2 0f 11 43 08       	movsd  %xmm0,0x8(%rbx)
  69151d:	f2 0f 11 5b 10       	movsd  %xmm3,0x10(%rbx)
  691522:	48 83 c4 20          	add    $0x20,%rsp
  691526:	5b                   	pop    %rbx
  691527:	41 5e                	pop    %r14
  691529:	41 5f                	pop    %r15
  69152b:	c3                   	ret

aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

0000000000691a30 <_ZNK3gfx3Rgb12maxComponentEv>:
  691a30:	8b 0f                	mov    (%rdi),%ecx
  691a32:	8b 47 04             	mov    0x4(%rdi),%eax
  691a35:	8b 57 08             	mov    0x8(%rdi),%edx
  691a38:	39 c1                	cmp    %eax,%ecx
  691a3a:	0f 4f c1             	cmovg  %ecx,%eax
  691a3d:	39 d0                	cmp    %edx,%eax
  691a3f:	0f 4e c2             	cmovle %edx,%eax
  691a42:	c3                   	ret
  691a43:	90                   	nop
  691a44:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  691a4b:	00 00 00 00 00

0000000000691a50 <_ZNK3gfx3Rgb12minComponentEv>:
  691a50:	8b 0f                	mov    (%rdi),%ecx
  691a52:	8b 47 04             	mov    0x4(%rdi),%eax
  691a55:	8b 57 08             	mov    0x8(%rdi),%edx
  691a58:	39 c1                	cmp    %eax,%ecx
  691a5a:	0f 4c c1             	cmovl  %ecx,%eax
  691a5d:	39 d0                	cmp    %edx,%eax
  691a5f:	0f 4d c2             	cmovge %edx,%eax
  691a62:	c3                   	ret

aseprite-reference:     file format elf64-x86-64

Contents of section .data.rel.ro:
 1cbafd8 e7fbf300 00000000 206a3c00 00000000  ........ j<.....
 1cbafe8 00000000 00000000                    ........

aseprite-reference:     file format elf64-x86-64

Contents of section .rodata:
 f7b3f8 00000000 00e06f40                    ......o@

aseprite-reference:     file format elf64-x86-64

Contents of section .rodata:
 f7b498 00000000 0000e03f                    .......?

aseprite-reference:     file format elf64-x86-64

Contents of section .rodata:
 f7c200 00000000 00e06f40 00000000 00e06f40  ......o@......o@
