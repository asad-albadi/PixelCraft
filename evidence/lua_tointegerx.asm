
aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000053eab0 <_Z14lua_tointegerxP9lua_StateiPi>:
  53eab0:	53                   	push   %rbx
  53eab1:	48 83 ec 10          	sub    $0x10,%rsp
  53eab5:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  53eabc:	00 00 
  53eabe:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  53eac3:	48 c7 04 24 00 00 00 	movq   $0x0,(%rsp)
  53eaca:	00 
  53eacb:	48 8b 4f 20          	mov    0x20(%rdi),%rcx
  53eacf:	85 f6                	test   %esi,%esi
  53ead1:	7e 22                	jle    53eaf5 <_Z14lua_tointegerxP9lua_StateiPi+0x45>
  53ead3:	89 f0                	mov    %esi,%eax
  53ead5:	48 c1 e0 04          	shl    $0x4,%rax
  53ead9:	48 03 01             	add    (%rcx),%rax
  53eadc:	48 3b 47 10          	cmp    0x10(%rdi),%rax
  53eae0:	72 3e                	jb     53eb20 <_Z14lua_tointegerxP9lua_StateiPi+0x70>
  53eae2:	48 8b 47 18          	mov    0x18(%rdi),%rax
  53eae6:	48 83 c0 50          	add    $0x50,%rax
  53eaea:	80 78 08 03          	cmpb   $0x3,0x8(%rax)
  53eaee:	74 36                	je     53eb26 <_Z14lua_tointegerxP9lua_StateiPi+0x76>
  53eaf0:	e9 97 00 00 00       	jmp    53eb8c <_Z14lua_tointegerxP9lua_StateiPi+0xdc>
  53eaf5:	81 fe d9 b9 f0 ff    	cmp    $0xfff0b9d9,%esi
  53eafb:	7d 18                	jge    53eb15 <_Z14lua_tointegerxP9lua_StateiPi+0x65>
  53eafd:	81 fe d8 b9 f0 ff    	cmp    $0xfff0b9d8,%esi
  53eb03:	75 4e                	jne    53eb53 <_Z14lua_tointegerxP9lua_StateiPi+0xa3>
  53eb05:	48 8b 47 18          	mov    0x18(%rdi),%rax
  53eb09:	48 83 c0 40          	add    $0x40,%rax
  53eb0d:	80 78 08 03          	cmpb   $0x3,0x8(%rax)
  53eb11:	74 13                	je     53eb26 <_Z14lua_tointegerxP9lua_StateiPi+0x76>
  53eb13:	eb 77                	jmp    53eb8c <_Z14lua_tointegerxP9lua_StateiPi+0xdc>
  53eb15:	48 63 c6             	movslq %esi,%rax
  53eb18:	48 c1 e0 04          	shl    $0x4,%rax
  53eb1c:	48 03 47 10          	add    0x10(%rdi),%rax
  53eb20:	80 78 08 03          	cmpb   $0x3,0x8(%rax)
  53eb24:	75 66                	jne    53eb8c <_Z14lua_tointegerxP9lua_StateiPi+0xdc>
  53eb26:	48 8b 00             	mov    (%rax),%rax
  53eb29:	48 89 04 24          	mov    %rax,(%rsp)
  53eb2d:	b8 01 00 00 00       	mov    $0x1,%eax
  53eb32:	48 85 d2             	test   %rdx,%rdx
  53eb35:	74 02                	je     53eb39 <_Z14lua_tointegerxP9lua_StateiPi+0x89>
  53eb37:	89 02                	mov    %eax,(%rdx)
  53eb39:	48 8b 04 24          	mov    (%rsp),%rax
  53eb3d:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  53eb44:	00 00 
  53eb46:	48 3b 4c 24 08       	cmp    0x8(%rsp),%rcx
  53eb4b:	75 5c                	jne    53eba9 <_Z14lua_tointegerxP9lua_StateiPi+0xf9>
  53eb4d:	48 83 c4 10          	add    $0x10,%rsp
  53eb51:	5b                   	pop    %rbx
  53eb52:	c3                   	ret
  53eb53:	48 8b 01             	mov    (%rcx),%rax
  53eb56:	80 78 08 66          	cmpb   $0x66,0x8(%rax)
  53eb5a:	75 86                	jne    53eae2 <_Z14lua_tointegerxP9lua_StateiPi+0x32>
  53eb5c:	b9 d8 b9 f0 ff       	mov    $0xfff0b9d8,%ecx
  53eb61:	29 f1                	sub    %esi,%ecx
  53eb63:	48 8b 00             	mov    (%rax),%rax
  53eb66:	44 0f b6 40 0a       	movzbl 0xa(%rax),%r8d
  53eb6b:	44 39 c1             	cmp    %r8d,%ecx
  53eb6e:	0f 87 6e ff ff ff    	ja     53eae2 <_Z14lua_tointegerxP9lua_StateiPi+0x32>
  53eb74:	b9 d7 b9 f0 ff       	mov    $0xfff0b9d7,%ecx
  53eb79:	29 f1                	sub    %esi,%ecx
  53eb7b:	48 c1 e1 04          	shl    $0x4,%rcx
  53eb7f:	48 01 c8             	add    %rcx,%rax
  53eb82:	48 83 c0 20          	add    $0x20,%rax
  53eb86:	80 78 08 03          	cmpb   $0x3,0x8(%rax)
  53eb8a:	74 9a                	je     53eb26 <_Z14lua_tointegerxP9lua_StateiPi+0x76>
  53eb8c:	48 89 e6             	mov    %rsp,%rsi
  53eb8f:	48 89 c7             	mov    %rax,%rdi
  53eb92:	48 89 d3             	mov    %rdx,%rbx
  53eb95:	ba 01 00 00 00       	mov    $0x1,%edx
  53eb9a:	e8 91 5d 01 00       	call   554930 <_Z14luaV_tointegerPK6TValuePx6F2Imod>
  53eb9f:	48 89 da             	mov    %rbx,%rdx
  53eba2:	48 85 d2             	test   %rdx,%rdx
  53eba5:	75 90                	jne    53eb37 <_Z14lua_tointegerxP9lua_StateiPi+0x87>
  53eba7:	eb 90                	jmp    53eb39 <_Z14lua_tointegerxP9lua_StateiPi+0x89>
  53eba9:	e8 32 7a c0 ff       	call   1465e0 <__stack_chk_fail@plt>
