
aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

0000000000554930 <_Z14luaV_tointegerPK6TValuePx6F2Imod>:
  554930:	55                   	push   %rbp
  554931:	41 57                	push   %r15
  554933:	41 56                	push   %r14
  554935:	53                   	push   %rbx
  554936:	48 83 ec 28          	sub    $0x28,%rsp
  55493a:	89 d5                	mov    %edx,%ebp
  55493c:	48 89 f3             	mov    %rsi,%rbx
  55493f:	49 89 fe             	mov    %rdi,%r14
  554942:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  554949:	00 00 
  55494b:	48 89 44 24 20       	mov    %rax,0x20(%rsp)
  554950:	0f b6 47 08          	movzbl 0x8(%rdi),%eax
  554954:	89 c1                	mov    %eax,%ecx
  554956:	80 e1 0f             	and    $0xf,%cl
  554959:	80 f9 04             	cmp    $0x4,%cl
  55495c:	75 36                	jne    554994 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x64>
  55495e:	49 8b 3e             	mov    (%r14),%rdi
  554961:	48 83 c7 18          	add    $0x18,%rdi
  554965:	4c 8d 7c 24 10       	lea    0x10(%rsp),%r15
  55496a:	4c 89 fe             	mov    %r15,%rsi
  55496d:	e8 9e 52 ff ff       	call   549c10 <_Z12luaO_str2numPKcP6TValue>
  554972:	49 8b 0e             	mov    (%r14),%rcx
  554975:	80 79 08 04          	cmpb   $0x4,0x8(%rcx)
  554979:	75 06                	jne    554981 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x51>
  55497b:	0f b6 49 0b          	movzbl 0xb(%rcx),%ecx
  55497f:	eb 04                	jmp    554985 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x55>
  554981:	48 8b 49 10          	mov    0x10(%rcx),%rcx
  554985:	48 ff c1             	inc    %rcx
  554988:	48 39 c8             	cmp    %rcx,%rax
  55498b:	4d 0f 44 f7          	cmove  %r15,%r14
  55498f:	41 0f b6 46 08       	movzbl 0x8(%r14),%eax
  554994:	3c 03                	cmp    $0x3,%al
  554996:	0f 84 83 00 00 00    	je     554a1f <_Z14luaV_tointegerPK6TValuePx6F2Imod+0xef>
  55499c:	0f b6 c8             	movzbl %al,%ecx
  55499f:	31 c0                	xor    %eax,%eax
  5549a1:	83 f9 13             	cmp    $0x13,%ecx
  5549a4:	0f 85 80 00 00 00    	jne    554a2a <_Z14luaV_tointegerPK6TValuePx6F2Imod+0xfa>
  5549aa:	f2 41 0f 10 06       	movsd  (%r14),%xmm0
  5549af:	f2 0f 11 44 24 08    	movsd  %xmm0,0x8(%rsp)
  5549b5:	e8 c6 2c bf ff       	call   147680 <floor@plt>
  5549ba:	f2 0f 10 4c 24 08    	movsd  0x8(%rsp),%xmm1
  5549c0:	66 0f 2e c8          	ucomisd %xmm0,%xmm1
  5549c4:	75 02                	jne    5549c8 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x98>
  5549c6:	7b 11                	jnp    5549d9 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0xa9>
  5549c8:	85 ed                	test   %ebp,%ebp
  5549ca:	74 79                	je     554a45 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x115>
  5549cc:	83 fd 02             	cmp    $0x2,%ebp
  5549cf:	75 08                	jne    5549d9 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0xa9>
  5549d1:	f2 0f 58 05 0f 6a a2 	addsd  0xa26a0f(%rip),%xmm0        # f7b3e8 <_ZTSN2ui13MenuSeparatorE+0x1b>
  5549d8:	00 
  5549d9:	f2 0f 10 0d b7 6c a2 	movsd  0xa26cb7(%rip),%xmm1        # f7b698 <_ZTSN2ui13MenuSeparatorE+0x2cb>
  5549e0:	00 
  5549e1:	66 0f 28 d0          	movapd %xmm0,%xmm2
  5549e5:	f2 0f c2 15 62 6c a2 	cmpltsd 0xa26c62(%rip),%xmm2        # f7b650 <_ZTSN2ui13MenuSeparatorE+0x283>
  5549ec:	00 01 
  5549ee:	f2 0f c2 c8 02       	cmplesd %xmm0,%xmm1
  5549f3:	66 0f 54 d1          	andpd  %xmm1,%xmm2
  5549f7:	66 0f 7e d0          	movd   %xmm2,%eax
  5549fb:	a8 01                	test   $0x1,%al
  5549fd:	74 08                	je     554a07 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0xd7>
  5549ff:	f2 48 0f 2c c8       	cvttsd2si %xmm0,%rcx
  554a04:	48 89 0b             	mov    %rcx,(%rbx)
  554a07:	0f b6 c0             	movzbl %al,%eax
  554a0a:	83 e0 01             	and    $0x1,%eax
  554a0d:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  554a14:	00 00 
  554a16:	48 3b 4c 24 20       	cmp    0x20(%rsp),%rcx
  554a1b:	74 1d                	je     554a3a <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x10a>
  554a1d:	eb 38                	jmp    554a57 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x127>
  554a1f:	49 8b 06             	mov    (%r14),%rax
  554a22:	48 89 03             	mov    %rax,(%rbx)
  554a25:	b8 01 00 00 00       	mov    $0x1,%eax
  554a2a:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  554a31:	00 00 
  554a33:	48 3b 4c 24 20       	cmp    0x20(%rsp),%rcx
  554a38:	75 1d                	jne    554a57 <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x127>
  554a3a:	48 83 c4 28          	add    $0x28,%rsp
  554a3e:	5b                   	pop    %rbx
  554a3f:	41 5e                	pop    %r14
  554a41:	41 5f                	pop    %r15
  554a43:	5d                   	pop    %rbp
  554a44:	c3                   	ret
  554a45:	89 e8                	mov    %ebp,%eax
  554a47:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  554a4e:	00 00 
  554a50:	48 3b 4c 24 20       	cmp    0x20(%rsp),%rcx
  554a55:	74 e3                	je     554a3a <_Z14luaV_tointegerPK6TValuePx6F2Imod+0x10a>
  554a57:	e8 84 1b bf ff       	call   1465e0 <__stack_chk_fail@plt>
  554a5c:	0f 1f 40 00          	nopl   0x0(%rax)
