# Elora HID interface 

Application to collect data and push it through USB to Elora split keyboard. For now it's just TSLA & VWRL stock price

on QMK side it's this codebase which receives and renders it:

- receiving through raw hid https://github.com/dzhibas/vial-qmk/blob/elora_raw_hid/keyboards/splitkb/elora/rev1/rev1.c#L225-L241
- drawing it https://github.com/dzhibas/vial-qmk/blob/elora_raw_hid/keyboards/splitkb/elora/rev1/rev1.c#L310-L314

Example:

![photo_2024-01-06 15 24 59](https://github.com/dzhibas/elora_hid/assets/400147/76730131-bc92-4ff5-8355-1202390ee4f3)
