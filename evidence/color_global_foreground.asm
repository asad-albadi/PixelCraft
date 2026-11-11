
/home/asad/.local/share/aseprite/1.3.18.6/aseprite:     file format elf64-x86-64


Disassembly of section .text:

000000000022cea0 <app::ColorBar::onFgColorButtonChange(app::Color const&)>:
  22cea0:	41 57                	push   %r15
  22cea2:	41 56                	push   %r14
  22cea4:	41 54                	push   %r12
  22cea6:	53                   	push   %rbx
  22cea7:	50                   	push   %rax
  22cea8:	80 bf b2 3f 00 00 00 	cmpb   $0x0,0x3fb2(%rdi)
  22ceaf:	0f 85 e1 00 00 00    	jne    22cf96 <app::ColorBar::onFgColorButtonChange(app::Color const&)+0xf6>
  22ceb5:	49 89 f6             	mov    %rsi,%r14
  22ceb8:	48 89 fb             	mov    %rdi,%rbx
  22cebb:	c6 87 b2 3f 00 00 01 	movb   $0x1,0x3fb2(%rdi)
  22cec2:	80 bf b1 3f 00 00 00 	cmpb   $0x0,0x3fb1(%rdi)
  22cec9:	0f 85 a6 00 00 00    	jne    22cf75 <app::ColorBar::onFgColorButtonChange(app::Color const&)+0xd5>
  22cecf:	c6 83 b1 3f 00 00 01 	movb   $0x1,0x3fb1(%rbx)
  22ced6:	e8 f5 58 fb ff       	call   1e27d0 <app::Preferences::instance()>
  22cedb:	49 89 c7             	mov    %rax,%r15
  22cede:	4c 8d a0 30 47 00 00 	lea    0x4730(%rax),%r12
  22cee5:	4c 89 e7             	mov    %r12,%rdi
  22cee8:	4c 89 f6             	mov    %r14,%rsi
  22ceeb:	e8 00 f5 f3 ff       	call   16c3f0 <app::Color::operator==(app::Color const&) const>
  22cef0:	84 c0                	test   %al,%al
  22cef2:	74 0a                	je     22cefe <app::ColorBar::onFgColorButtonChange(app::Color const&)+0x5e>
  22cef4:	41 c6 87 58 47 00 00 	movb   $0x1,0x4758(%r15)
  22cefb:	01
  22cefc:	eb 70                	jmp    22cf6e <app::ColorBar::onFgColorButtonChange(app::Color const&)+0xce>
  22cefe:	49 8d bf 88 46 00 00 	lea    0x4688(%r15),%rdi
  22cf05:	4c 89 f6             	mov    %r14,%rsi
  22cf08:	e8 73 f6 f5 ff       	call   18c580 <std::enable_if<std::is_void<void>::value, void>::type obs::signal<void (app::Color const&), obs::default_list>::operator()<void, app::Color const&>(app::Color const&)>
  22cf0d:	49 8b bf 78 46 00 00 	mov    0x4678(%r15),%rdi
  22cf14:	48 85 ff             	test   %rdi,%rdi
  22cf17:	74 09                	je     22cf22 <app::ColorBar::onFgColorButtonChange(app::Color const&)+0x82>
  22cf19:	48 83 c7 08          	add    $0x8,%rdi
  22cf1d:	e8 be 0b f2 ff       	call   14dae0 <std::enable_if<std::is_void<void>::value, void>::type obs::signal<void (), obs::default_list>::operator()<void>()>
  22cf22:	49 8b 46 20          	mov    0x20(%r14),%rax
  22cf26:	49 89 44 24 20       	mov    %rax,0x20(%r12)
  22cf2b:	41 0f 10 06          	movups (%r14),%xmm0
  22cf2f:	41 0f 10 4e 10       	movups 0x10(%r14),%xmm1
  22cf34:	41 0f 11 4c 24 10    	movups %xmm1,0x10(%r12)
  22cf3a:	41 0f 11 04 24       	movups %xmm0,(%r12)
  22cf3f:	41 c6 87 58 47 00 00 	movb   $0x1,0x4758(%r15)
  22cf46:	01
  22cf47:	4c 89 ff             	mov    %r15,%rdi
  22cf4a:	48 81 c7 c8 46 00 00 	add    $0x46c8,%rdi
  22cf51:	4c 89 f6             	mov    %r14,%rsi
  22cf54:	e8 27 f6 f5 ff       	call   18c580 <std::enable_if<std::is_void<void>::value, void>::type obs::signal<void (app::Color const&), obs::default_list>::operator()<void, app::Color const&>(app::Color const&)>
  22cf59:	49 8b bf 78 46 00 00 	mov    0x4678(%r15),%rdi
  22cf60:	48 85 ff             	test   %rdi,%rdi
  22cf63:	74 09                	je     22cf6e <app::ColorBar::onFgColorButtonChange(app::Color const&)+0xce>
  22cf65:	48 83 c7 48          	add    $0x48,%rdi
  22cf69:	e8 72 0b f2 ff       	call   14dae0 <std::enable_if<std::is_void<void>::value, void>::type obs::signal<void (), obs::default_list>::operator()<void>()>
  22cf6e:	c6 83 b1 3f 00 00 00 	movb   $0x0,0x3fb1(%rbx)
  22cf75:	48 8b 93 f0 39 00 00 	mov    0x39f0(%rbx),%rdx
  22cf7c:	4c 89 f6             	mov    %r14,%rsi
  22cf7f:	e8 1c 2a 00 00       	call   22f9a0 <app::ColorBar::updateWarningIcon(app::Color const&, ui::Button*)>
  22cf84:	48 89 df             	mov    %rbx,%rdi
  22cf87:	4c 89 f6             	mov    %r14,%rsi
  22cf8a:	e8 b1 0d 00 00       	call   22dd40 <app::ColorBar::onColorButtonChange(app::Color const&)>
  22cf8f:	c6 83 b2 3f 00 00 00 	movb   $0x0,0x3fb2(%rbx)
  22cf96:	48 83 c4 08          	add    $0x8,%rsp
  22cf9a:	5b                   	pop    %rbx
  22cf9b:	41 5c                	pop    %r12
  22cf9d:	41 5e                	pop    %r14
  22cf9f:	41 5f                	pop    %r15
  22cfa1:	c3                   	ret
  22cfa2:	c6 83 b1 3f 00 00 00 	movb   $0x0,0x3fb1(%rbx)
  22cfa9:	c6 83 b2 3f 00 00 00 	movb   $0x0,0x3fb2(%rbx)
  22cfb0:	48 89 c7             	mov    %rax,%rdi
  22cfb3:	e8 28 9c f1 ff       	call   146be0 <_Unwind_Resume@plt>
  22cfb8:	c6 83 b2 3f 00 00 00 	movb   $0x0,0x3fb2(%rbx)
  22cfbf:	48 89 c7             	mov    %rax,%rdi
  22cfc2:	e8 19 9c f1 ff       	call   146be0 <_Unwind_Resume@plt>
  22cfc7:	90                   	nop
  22cfc8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  22cfcf:	00
