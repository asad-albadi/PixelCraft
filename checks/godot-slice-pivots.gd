extends SceneTree


func _initialize() -> void:
	run.call_deferred()


func fail(message: String) -> void:
	push_error(message)
	quit.call_deferred(1)


func run() -> void:
	var sheet: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://atlas.json"))
	var frames := load("res://atlas.tres") as SpriteFrames
	if frames == null or frames.get_frame_count("default") != 3:
		fail("Could not load the three frames from PixelCraft's SpriteFrames resource")
		return
	var slice: Dictionary = sheet.meta.slices[0]
	if slice.name != "anchor" or slice.keys.size() != 2 or int(sheet.meta.padding) != 6:
		fail("Expected two anchor keys and six pixels of scaled padding")
		return

	var viewport := SubViewport.new()
	viewport.size = Vector2i(48, 32)
	viewport.transparent_bg = true
	viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	root.add_child(viewport)

	var sprite := AnimatedSprite2D.new()
	sprite.sprite_frames = frames
	sprite.animation = "default"
	sprite.centered = false
	sprite.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	var anchor := Vector2i(19, 13)
	sprite.position = anchor
	viewport.add_child(sprite)

	# Expected positions come from the three painted rectangles, relative to the
	# white marker in each source frame, at twice the original pixel size.
	var expected := [
		Rect2i(-2, -2, 6, 6),
		Rect2i(-2, -2, 8, 6),
		Rect2i(0, -4, 4, 6),
	]
	var inks := [Color.RED, Color.GREEN, Color.BLUE]
	var key: Dictionary = {}
	for i in range(3):
		for candidate in slice.keys:
			if int(candidate.frame) == i:
				key = candidate
		if key.is_empty():
			fail("No active slice key for exported frame %d" % i)
			return

		var frame: Dictionary = sheet.frames[i]
		var texture := frames.get_frame_texture("default", i) as AtlasTexture
		var region := Rect2(frame.frame.x, frame.frame.y, frame.frame.w, frame.frame.h)
		var trim := Vector2(frame.spriteSourceSize.x, frame.spriteSourceSize.y)
		if texture == null or texture.region != region or texture.margin.position != trim:
			fail("Godot atlas region or trim offset disagrees with JSON for frame %d" % i)
			return
		var source_pivot := Vector2(key.bounds[0], key.bounds[1]) + Vector2(key.pivot[0], key.pivot[1])
		# AtlasTexture.margin restores the trim offset, so use the source pivot.
		# For a bare cropped region, offset by -(source_pivot - trim) instead.
		sprite.offset = -source_pivot
		sprite.frame = i
		await RenderingServer.frame_post_draw
		var image := viewport.get_texture().get_image()
		for y in range(image.get_height()):
			for x in range(image.get_width()):
				var point := Vector2i(x, y) - anchor
				var target := Color.TRANSPARENT
				if Rect2i(0, 0, 2, 2).has_point(point):
					target = Color.WHITE
				elif expected[i].has_point(point):
					target = inks[i]
				var actual := image.get_pixel(x, y)
				if actual.a != target.a or (target.a != 0 and actual != target):
					fail("Frame %d pixel %s was %s, expected %s" % [i, Vector2i(x, y), actual, target])
					return
		print("Frame %d aligned at %s using slice key %d" % [i, anchor, int(key.frame)])
	print("PixelCraft Godot slice pivot check passed (3 rendered frames, 2 keys)")
	viewport.queue_free()
	quit.call_deferred()
