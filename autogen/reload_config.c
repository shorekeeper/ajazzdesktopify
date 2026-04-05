int reload_device_config()
{
  _BYTE *v0; // r1
  int result; // r0

  v0 = g_config_sram_ptr;
  *((_BYTE *)g_config_sram_ptr + 5) = cfg_flash_param1;
  v0[6] = cfg_flash_param2;
  v0[7] = cfg_flash_param3;
  *(_BYTE *)g_config_field_B = cfg_flash_param4;
  *(_BYTE *)g_config_field_E = cfg_flash_param5;
  result = (unsigned __int8)cfg_flash_mode;
  *(_BYTE *)g_config_field_mode = cfg_flash_mode;
  return result;
}