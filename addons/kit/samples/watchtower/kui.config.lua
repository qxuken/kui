return {
  name = "Watchtower",
  id = "dev.kui.samples.watchtower",
  version = "1.0.0",
  entry = "main.tl",
  size = { w = 1320, h = 880 },
  -- the headline cards wrap two a line and the charts stack as it narrows
  -- (ADR 0069); below this the services table's columns crowd
  min_size = { w = 880, h = 680 },
}
