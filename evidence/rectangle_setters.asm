
aseprite:     file format elf64-x86-64


Disassembly of section .text:

00000000003ea6d0 <_ZN3app6script12_GLOBAL__N_115Rectangle_set_xEP9lua_State>:
  3ea6d0:	41 56                	push   %r14
  3ea6d2:	53                   	push   %rbx
  3ea6d3:	50                   	push   %rax
  3ea6d4:	48 89 fb             	mov    %rdi,%rbx
  3ea6d7:	48 8d 15 22 65 b5 00 	lea    0xb56522(%rip),%rdx        # f40c00 <_ZL16gUnaryActiveEdge+0xd49c>
  3ea6de:	be 01 00 00 00       	mov    $0x1,%esi
  3ea6e3:	e8 98 4c 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3ea6e8:	49 89 c6             	mov    %rax,%r14
  3ea6eb:	48 89 df             	mov    %rbx,%rdi
  3ea6ee:	be 02 00 00 00       	mov    $0x2,%esi
  3ea6f3:	31 d2                	xor    %edx,%edx
  3ea6f5:	e8 b6 43 15 00       	call   53eab0 <_Z14lua_tointegerxP9lua_StateiPi>
  3ea6fa:	41 89 06             	mov    %eax,(%r14)
  3ea6fd:	31 c0                	xor    %eax,%eax
  3ea6ff:	48 83 c4 08          	add    $0x8,%rsp
  3ea703:	5b                   	pop    %rbx
  3ea704:	41 5e                	pop    %r14
  3ea706:	c3                   	ret
  3ea707:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  3ea70e:	00 00

00000000003ea710 <_ZN3app6script12_GLOBAL__N_115Rectangle_get_yEP9lua_State>:
  3ea710:	53                   	push   %rbx
  3ea711:	48 89 fb             	mov    %rdi,%rbx
  3ea714:	48 8d 15 e5 64 b5 00 	lea    0xb564e5(%rip),%rdx        # f40c00 <_ZL16gUnaryActiveEdge+0xd49c>
  3ea71b:	be 01 00 00 00       	mov    $0x1,%esi
  3ea720:	e8 5b 4c 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3ea725:	48 63 70 04          	movslq 0x4(%rax),%rsi
  3ea729:	48 89 df             	mov    %rbx,%rdi
  3ea72c:	e8 df 4a 15 00       	call   53f210 <_Z15lua_pushintegerP9lua_Statex>
  3ea731:	b8 01 00 00 00       	mov    $0x1,%eax
  3ea736:	5b                   	pop    %rbx
  3ea737:	c3                   	ret
  3ea738:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3ea73f:	00

00000000003ea740 <_ZN3app6script12_GLOBAL__N_115Rectangle_set_yEP9lua_State>:
  3ea740:	41 56                	push   %r14
  3ea742:	53                   	push   %rbx
  3ea743:	50                   	push   %rax
  3ea744:	48 89 fb             	mov    %rdi,%rbx
  3ea747:	48 8d 15 b2 64 b5 00 	lea    0xb564b2(%rip),%rdx        # f40c00 <_ZL16gUnaryActiveEdge+0xd49c>
  3ea74e:	be 01 00 00 00       	mov    $0x1,%esi
  3ea753:	e8 28 4c 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3ea758:	49 89 c6             	mov    %rax,%r14
  3ea75b:	48 89 df             	mov    %rbx,%rdi
  3ea75e:	be 02 00 00 00       	mov    $0x2,%esi
  3ea763:	31 d2                	xor    %edx,%edx
  3ea765:	e8 46 43 15 00       	call   53eab0 <_Z14lua_tointegerxP9lua_StateiPi>
  3ea76a:	41 89 46 04          	mov    %eax,0x4(%r14)
  3ea76e:	31 c0                	xor    %eax,%eax
  3ea770:	48 83 c4 08          	add    $0x8,%rsp
  3ea774:	5b                   	pop    %rbx
  3ea775:	41 5e                	pop    %r14
  3ea777:	c3                   	ret
  3ea778:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3ea77f:	00

00000000003ea780 <_ZN3app6script12_GLOBAL__N_119Rectangle_get_widthEP9lua_State>:
  3ea780:	53                   	push   %rbx
  3ea781:	48 89 fb             	mov    %rdi,%rbx
  3ea784:	48 8d 15 75 64 b5 00 	lea    0xb56475(%rip),%rdx        # f40c00 <_ZL16gUnaryActiveEdge+0xd49c>
  3ea78b:	be 01 00 00 00       	mov    $0x1,%esi
  3ea790:	e8 eb 4b 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3ea795:	48 63 70 08          	movslq 0x8(%rax),%rsi
  3ea799:	48 89 df             	mov    %rbx,%rdi
  3ea79c:	e8 6f 4a 15 00       	call   53f210 <_Z15lua_pushintegerP9lua_Statex>
  3ea7a1:	b8 01 00 00 00       	mov    $0x1,%eax
  3ea7a6:	5b                   	pop    %rbx
  3ea7a7:	c3                   	ret
  3ea7a8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3ea7af:	00

00000000003ea7b0 <_ZN3app6script12_GLOBAL__N_119Rectangle_set_widthEP9lua_State>:
  3ea7b0:	41 56                	push   %r14
  3ea7b2:	53                   	push   %rbx
  3ea7b3:	50                   	push   %rax
  3ea7b4:	48 89 fb             	mov    %rdi,%rbx
  3ea7b7:	48 8d 15 42 64 b5 00 	lea    0xb56442(%rip),%rdx        # f40c00 <_ZL16gUnaryActiveEdge+0xd49c>
  3ea7be:	be 01 00 00 00       	mov    $0x1,%esi
  3ea7c3:	e8 b8 4b 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3ea7c8:	49 89 c6             	mov    %rax,%r14
  3ea7cb:	48 89 df             	mov    %rbx,%rdi
  3ea7ce:	be 02 00 00 00       	mov    $0x2,%esi
  3ea7d3:	31 d2                	xor    %edx,%edx
  3ea7d5:	e8 d6 42 15 00       	call   53eab0 <_Z14lua_tointegerxP9lua_StateiPi>
  3ea7da:	41 89 46 08          	mov    %eax,0x8(%r14)
  3ea7de:	31 c0                	xor    %eax,%eax
  3ea7e0:	48 83 c4 08          	add    $0x8,%rsp
  3ea7e4:	5b                   	pop    %rbx
  3ea7e5:	41 5e                	pop    %r14
  3ea7e7:	c3                   	ret
  3ea7e8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3ea7ef:	00

00000000003ea7f0 <_ZN3app6script12_GLOBAL__N_120Rectangle_get_heightEP9lua_State>:
  3ea7f0:	53                   	push   %rbx
  3ea7f1:	48 89 fb             	mov    %rdi,%rbx
  3ea7f4:	48 8d 15 05 64 b5 00 	lea    0xb56405(%rip),%rdx        # f40c00 <_ZL16gUnaryActiveEdge+0xd49c>
  3ea7fb:	be 01 00 00 00       	mov    $0x1,%esi
  3ea800:	e8 7b 4b 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3ea805:	48 63 70 0c          	movslq 0xc(%rax),%rsi
  3ea809:	48 89 df             	mov    %rbx,%rdi
  3ea80c:	e8 ff 49 15 00       	call   53f210 <_Z15lua_pushintegerP9lua_Statex>
  3ea811:	b8 01 00 00 00       	mov    $0x1,%eax
  3ea816:	5b                   	pop    %rbx
  3ea817:	c3                   	ret
  3ea818:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3ea81f:	00

00000000003ea820 <_ZN3app6script12_GLOBAL__N_120Rectangle_set_heightEP9lua_State>:
  3ea820:	41 56                	push   %r14
  3ea822:	53                   	push   %rbx
  3ea823:	50                   	push   %rax
  3ea824:	48 89 fb             	mov    %rdi,%rbx
  3ea827:	48 8d 15 d2 63 b5 00 	lea    0xb563d2(%rip),%rdx        # f40c00 <_ZL16gUnaryActiveEdge+0xd49c>
  3ea82e:	be 01 00 00 00       	mov    $0x1,%esi
  3ea833:	e8 48 4b 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3ea838:	49 89 c6             	mov    %rax,%r14
  3ea83b:	48 89 df             	mov    %rbx,%rdi
  3ea83e:	be 02 00 00 00       	mov    $0x2,%esi
  3ea843:	31 d2                	xor    %edx,%edx
  3ea845:	e8 66 42 15 00       	call   53eab0 <_Z14lua_tointegerxP9lua_StateiPi>
  3ea84a:	41 89 46 0c          	mov    %eax,0xc(%r14)
  3ea84e:	31 c0                	xor    %eax,%eax
  3ea850:	48 83 c4 08          	add    $0x8,%rsp
  3ea854:	5b                   	pop    %rbx
  3ea855:	41 5e                	pop    %r14
  3ea857:	c3                   	ret
  3ea858:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3ea85f:	00


aseprite:     file format elf64-x86-64


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
  53ebae:	66 90                	xchg   %ax,%ax

000000000053ebb0 <_Z13lua_tobooleanP9lua_Statei>:
  53ebb0:	48 8b 4f 20          	mov    0x20(%rdi),%rcx
  53ebb4:	85 f6                	test   %esi,%esi
  53ebb6:	7e 19                	jle    53ebd1 <_Z13lua_tobooleanP9lua_Statei+0x21>
  53ebb8:	89 f0                	mov    %esi,%eax
  53ebba:	48 c1 e0 04          	shl    $0x4,%rax
  53ebbe:	48 03 01             	add    (%rcx),%rax
  53ebc1:	48 3b 47 10          	cmp    0x10(%rdi),%rax
  53ebc5:	72 2f                	jb     53ebf6 <_Z13lua_tobooleanP9lua_Statei+0x46>
  53ebc7:	48 8b 47 18          	mov    0x18(%rdi),%rax
  53ebcb:	48 83 c0 50          	add    $0x50,%rax
  53ebcf:	eb 25                	jmp    53ebf6 <_Z13lua_tobooleanP9lua_Statei+0x46>
  53ebd1:	81 fe d9 b9 f0 ff    	cmp    $0xfff0b9d9,%esi
  53ebd7:	7d 12                	jge    53ebeb <_Z13lua_tobooleanP9lua_Statei+0x3b>
  53ebd9:	81 fe d8 b9 f0 ff    	cmp    $0xfff0b9d8,%esi
  53ebdf:	75 29                	jne    53ec0a <_Z13lua_tobooleanP9lua_Statei+0x5a>
  53ebe1:	48 8b 47 18          	mov    0x18(%rdi),%rax
  53ebe5:	48 83 c0 40          	add    $0x40,%rax
  53ebe9:	eb 0b                	jmp    53ebf6 <_Z13lua_tobooleanP9lua_Statei+0x46>
  53ebeb:	48 63 c6             	movslq %esi,%rax
  53ebee:	48 c1 e0 04          	shl    $0x4,%rax
  53ebf2:	48 03 47 10          	add    0x10(%rdi),%rax
  53ebf6:	0f b6 40 08          	movzbl 0x8(%rax),%eax
  53ebfa:	3c 01                	cmp    $0x1,%al
  53ebfc:	0f 95 c1             	setne  %cl
  53ebff:	a8 0f                	test   $0xf,%al
  53ec01:	0f 95 c0             	setne  %al
  53ec04:	20 c8                	and    %cl,%al
  53ec06:	0f b6 c0             	movzbl %al,%eax
  53ec09:	c3                   	ret
  53ec0a:	48 8b 01             	mov    (%rcx),%rax
  53ec0d:	80                   	.byte 0x80
  53ec0e:	78 08                	js     53ec18 <_Z13lua_tobooleanP9lua_Statei+0x68>


aseprite:     file format elf64-x86-64


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
