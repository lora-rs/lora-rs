cmake_minimum_required(VERSION 3.10)

project(smtc-modem-cores)

set(SMTC_MODEM_CORE_DIR "SWL2001/lbm_lib/smtc_modem_core")

include_directories(
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src
  ${SMTC_MODEM_CORE_DIR}/smtc_ral/src/
)

# Add library using the variable
add_library(smtc-modem-cores STATIC
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src/sx126x.c
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src/sx126x.h
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src/sx126x_hal.h
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src/sx126x_lr_fhss.c
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src/sx126x_lr_fhss.h
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src/lr_fhss_mac.c
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src/lr_fhss_mac.h
  ${SMTC_MODEM_CORE_DIR}/radio_drivers/sx126x_driver/src/lr_fhss_v1_base_types.h
  ${SMTC_MODEM_CORE_DIR}/smtc_ral/src/ral_sx126x.c
  ${SMTC_MODEM_CORE_DIR}/smtc_ralf/src/ralf_sx126x.c
)


install(TARGETS smtc-modem-cores
        DESTINATION lib
  )
