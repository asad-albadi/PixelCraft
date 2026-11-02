function init(plugin)
  plugin:newCommand {
    id = "paint-mark",
    title = "Paint a small mark",
    onclick = function()
      app.transaction(function()
        local image = app.activeCel.image
        image:drawPixel(0, 0, app.pixelColor.rgba(12, 34, 56, 255))
      end)
    end
  }
end
