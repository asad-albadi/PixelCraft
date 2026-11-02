
aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

0000000000416140 <_ZN3app3cmd15SetLayerOpacityC1EPN3doc10LayerImageEi>:
  416140:	55                   	push   %rbp
  416141:	41 56                	push   %r14
  416143:	53                   	push   %rbx
  416144:	89 d5                	mov    %edx,%ebp
  416146:	49 89 f6             	mov    %rsi,%r14
  416149:	48 89 fb             	mov    %rdi,%rbx
  41614c:	e8 df 06 ff ff       	call   406830 <_ZN3app3CmdC1Ev>
  416151:	48 8d 7b 10          	lea    0x10(%rbx),%rdi
  416155:	4c 89 f6             	mov    %r14,%rsi
  416158:	e8 03 50 00 00       	call   41b160 <_ZN3app3cmd9WithLayerC1EPN3doc5LayerE>
  41615d:	48 8d 05 c4 a2 8a 01 	lea    0x18aa2c4(%rip),%rax        # 1cc0428 <_ZTVN3app3cmd15SetLayerOpacityE+0x10>
  416164:	48 89 03             	mov    %rax,(%rbx)
  416167:	41 8b 86 b8 00 00 00 	mov    0xb8(%r14),%eax
  41616e:	89 43 14             	mov    %eax,0x14(%rbx)
  416171:	89 6b 18             	mov    %ebp,0x18(%rbx)
  416174:	5b                   	pop    %rbx
  416175:	41 5e                	pop    %r14
  416177:	5d                   	pop    %rbp
  416178:	c3                   	ret
  416179:	49 89 c6             	mov    %rax,%r14
  41617c:	48 89 df             	mov    %rbx,%rdi
  41617f:	e8 b8 06 ff ff       	call   40683c <_ZN3app3CmdD1Ev>
  416184:	4c 89 f7             	mov    %r14,%rdi
  416187:	e8 54 0a d3 ff       	call   146be0 <_Unwind_Resume@plt>
  41618c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000416190 <_ZN3app3cmd15SetLayerOpacity9onExecuteEv>:
  416190:	41 56                	push   %r14
  416192:	53                   	push   %rbx
  416193:	50                   	push   %rax
  416194:	48 89 fb             	mov    %rdi,%rbx
  416197:	4c 8d 77 10          	lea    0x10(%rdi),%r14
  41619b:	4c 89 f7             	mov    %r14,%rdi
  41619e:	e8 dd 4f 00 00       	call   41b180 <_ZN3app3cmd9WithLayer5layerEv>
  4161a3:	8b 4b 18             	mov    0x18(%rbx),%ecx
  4161a6:	89 88 b8 00 00 00    	mov    %ecx,0xb8(%rax)
  4161ac:	4c 89 f7             	mov    %r14,%rdi
  4161af:	e8 cc 4f 00 00       	call   41b180 <_ZN3app3cmd9WithLayer5layerEv>
  4161b4:	ff 40 14             	incl   0x14(%rax)
  4161b7:	48 83 c4 08          	add    $0x8,%rsp
  4161bb:	5b                   	pop    %rbx
  4161bc:	41 5e                	pop    %r14
  4161be:	c3                   	ret
