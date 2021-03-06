/*
 * ASIX AX88179 based USB 3.0 Ethernet Devices
 * Copyright (C) 2003-2005 David Hollis <dhollis@davehollis.com>
 * Copyright (C) 2005 Phil Chang <pchang23@sbcglobal.net>
 * Copyright (c) 2002-2003 TiVo Inc.
 * Ported to Rust 2020 by David Hewitt.
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307  USA
 */

#![no_std]
#![feature(alloc_prelude)]
#![feature(format_args_capture)]

extern crate alloc;

use alloc::prelude::v1::*;
use core::mem::{transmute, zeroed, MaybeUninit};
use core::prelude::v1::*;

use linux_kernel_module::bindings::{
    __kbuild_modname, __this_module, driver_info, gfp_t, msleep, pm_message_t, sk_buff, urb,
    usb_deregister, usb_device_id, usb_driver, usb_interface, usb_register_driver, usbnet,
    usbnet_disconnect, usbnet_get_endpoints, usbnet_probe, usbnet_write_cmd, usbnet_write_cmd_nopm,
    ETH_ALEN, FLAG_AVOID_UNLINK_URBS, FLAG_ETHER, FLAG_FRAMING_AX, USB_DEVICE_ID_MATCH_DEVICE,
    USB_DIR_OUT, USB_RECIP_DEVICE, USB_TYPE_VENDOR, usbnet_read_cmd, usbnet_read_cmd_nopm,
    USB_DIR_IN,
};
use linux_kernel_module::c_types::{c_int, c_uchar, c_void};
use linux_kernel_module::{println, Error, KernelResult};

use nudge::unlikely;

#[allow(non_camel_case_types)]
struct ax88179_178a_module {
    _registration: DriverRegistration,
}

impl linux_kernel_module::KernelModule for ax88179_178a_module {
    fn init() -> linux_kernel_module::KernelResult<Self> {
        println!("Loading ax88179_178a");

        Ok(ax88179_178a_module {
            _registration: DriverRegistration::new(get_driver_info()),
        })
    }
}

impl Drop for ax88179_178a_module {
    fn drop(&mut self) {
        println!("Unloading ax88179_178a");
    }
}

// Holder on driver information
struct DriverRegistration {
    inner: Box<usb_driver>,
}

impl DriverRegistration {
    pub fn new(driver: usb_driver) -> Self {
        let mut inner = Box::new(driver);
        unsafe {
            usb_register_driver(
                inner.as_mut() as _,
                &mut __this_module as _,
                __kbuild_modname.as_ptr() as _,
            );
        }
        Self { inner }
    }
}

impl Drop for DriverRegistration {
    fn drop(&mut self) {
        unsafe {
            usb_deregister(self.inner.as_mut() as _);
        }
    }
}

// Never allow access of innards until drop
unsafe impl Sync for DriverRegistration {}

// #define RX_SKB_COPY
// const DRIVER_VERSION: &[u8; 6] = b"1.20.0";
const DRIVER_DESCRIPTION: &[u8; 46] = b"ASIX AX88179_178A USB 2.0/3.0 Ethernet Devices";
const DRIVER_AUTHOR: &[u8; 25] = b"ax88179_178a contributors";
const DRIVER_LICENSE: &[u8; 3] = b"GPL";

// #define MASK_WAKEUP_EVENT_4_SEC		0x01
// #define MASK_WAKEUP_EVENT_8_SEC		0x02
// #define MASK_WAKEUP_EVENT_TIMER		MASK_WAKEUP_EVENT_4_SEC

// #define AX88179_PHY_ID			0x03
// #define AX_MCAST_FILTER_SIZE		8
// #define AX_MAX_MCAST			64
// #define AX_EEPROM_LEN			0x40
// #define AX_RX_CHECKSUM			1
// #define AX_TX_CHECKSUM			2

const AX_BULKIN_24K: u8 = 0x18;
const AX_ACCESS_MAC: u8 = 0x01;
const AX_ACCESS_PHY: u8 = 0x02;
const AX_ACCESS_WAKEUP: u8 = 0x03;
const AX_ACCESS_EEPROM: u8 = 0x04;
const AX_ACCESS_EFUSE: u8 = 0x05;
const AX_RELOAD_EEPROM_EFUSE: u8 = 0x06;
const AX_WRITE_EFUSE_EN: u8 = 0x09;
const AX_WRITE_EFUSE_DIS: u8 = 0x0A;
const AX_ACCESS_MFAB: u8 = 0x10;

// #define PHYSICAL_LINK_STATUS		0x02
// 	#define	AX_USB_SS		0x04
// 	#define	AX_USB_HS		0x02
// 	#define	AX_USB_FS		0x01

// #define GENERAL_STATUS			0x03
// /* Check AX88179 version. UA1:Bit2 = 0,  UA2:Bit2 = 1 */
// 	#define	AX_SECLD		0x04

// #define AX_SROM_ADDR			0x07
// #define AX_SROM_CMD			0x0a
// 	#define EEP_RD			0x04	/* EEprom read command */
// 	#define EEP_WR			0x08	/* EEprom write command */
// 	#define EEP_BUSY		0x10	/* EEprom access module busy */
// #define AX_SROM_DATA_LOW		0x08
// #define AX_SROM_DATA_HIGH		0x09

// #define AX_RX_CTL			0x0b
// 	#define AX_RX_CTL_DROPCRCERR		0x0100 /* Drop CRC error packet */
// 	#define AX_RX_CTL_IPE			0x0200 /* Enable IP header in receive buffer aligned on 32-bit aligment */
// 	#define AX_RX_CTL_TXPADCRC		0x0400 /* checksum value in rx header 3 */
// 	#define AX_RX_CTL_START			0x0080 /* Ethernet MAC start */
// 	#define AX_RX_CTL_AP			0x0020 /* Accept physcial address from Multicast array */
// 	#define AX_RX_CTL_AM			0x0010 /* Accetp Brocadcast frames*/
// 	#define AX_RX_CTL_AB			0x0008 /* HW auto-added 8-bytes data when meet USB bulk in transfer boundary (1024/512/64)*/
// 	#define AX_RX_CTL_HA8B			0x0004
// 	#define AX_RX_CTL_AMALL			0x0002 /* Accetp all multicast frames */
// 	#define AX_RX_CTL_PRO			0x0001 /* Promiscuous Mode */
// 	#define AX_RX_CTL_STOP			0x0000 /* Stop MAC */
const AX_NODE_ID: u16 = 0x10;
// #define AX_MULTI_FILTER_ARRY		0x16

// #define AX_MEDIUM_STATUS_MODE			0x22
// 	#define AX_MEDIUM_GIGAMODE	0x01
// 	#define AX_MEDIUM_FULL_DUPLEX	0x02
// //	#define AX_MEDIUM_ALWAYS_ONE	0x04
// 	#define AX_MEDIUM_RXFLOW_CTRLEN	0x10
// 	#define AX_MEDIUM_TXFLOW_CTRLEN	0x20
// 	#define AX_MEDIUM_RECEIVE_EN	0x100
// 	#define AX_MEDIUM_PS		0x200
// 	#define AX_MEDIUM_JUMBO_EN	0x8040

// #define AX_MONITOR_MODE			0x24
// 	#define AX_MONITOR_MODE_RWLC		0x02
// 	#define AX_MONITOR_MODE_RWMP		0x04
// 	#define AX_MONITOR_MODE_RWWF		0x08
// 	#define AX_MONITOR_MODE_RW_FLAG		0x10
// 	#define AX_MONITOR_MODE_PMEPOL		0x20
// 	#define AX_MONITOR_MODE_PMETYPE		0x40

// #define AX_GPIO_CTRL			0x25
// 	#define AX_GPIO_CTRL_GPIO3EN		0x80
// 	#define AX_GPIO_CTRL_GPIO2EN		0x40
// 	#define AX_GPIO_CTRL_GPIO1EN		0x20

const AX_PHYPWR_RSTCTL: u16 = 0x26;
const AX_PHYPWR_RSTCTL_BZ: u16 = 0x0010;
const AX_PHYPWR_RSTCTL_IPRL: u16 = 0x0020;
const AX_PHYPWR_RSTCTL_AUTODETACH: u16 = 0x1000;

// #define AX_RX_BULKIN_QCTRL		0x2e
// 	#define AX_RX_BULKIN_QCTRL_TIME		0x01
// 	#define AX_RX_BULKIN_QCTRL_IFG		0x02
// 	#define AX_RX_BULKIN_QCTRL_SIZE		0x04

// #define AX_RX_BULKIN_QTIMR_LOW		0x2f
// #define AX_RX_BULKIN_QTIMR_HIGH			0x30
// #define AX_RX_BULKIN_QSIZE			0x31
// #define AX_RX_BULKIN_QIFG			0x32

const AX_CLK_SELECT: u16 = 0x33;
const AX_CLK_SELECT_BCS: u8 = 0x01;
const AX_CLK_SELECT_ACS: u8 = 0x02;
const AX_CLK_SELECT_ACSREQ: u8 = 0x10;
const AX_CLK_SELECT_ULR: u8 = 0x08;

// #define AX_RXCOE_CTL			0x34
// 	#define AX_RXCOE_IP			0x01
// 	#define AX_RXCOE_TCP			0x02
// 	#define AX_RXCOE_UDP			0x04
// 	#define AX_RXCOE_ICMP			0x08
// 	#define AX_RXCOE_IGMP			0x10
// 	#define AX_RXCOE_TCPV6			0x20
// 	#define AX_RXCOE_UDPV6			0x40
// 	#define AX_RXCOE_ICMV6			0x80

// #if LINUX_VERSION_CODE > KERNEL_VERSION(2, 6, 22)
// 	#define AX_RXCOE_DEF_CSUM	(AX_RXCOE_IP	| AX_RXCOE_TCP  | \
// 					 AX_RXCOE_UDP	| AX_RXCOE_ICMV6 | \
// 					 AX_RXCOE_TCPV6	| AX_RXCOE_UDPV6)
// #else
// 	#define AX_RXCOE_DEF_CSUM	(AX_RXCOE_IP	| AX_RXCOE_TCP | \
// 					 AX_RXCOE_UDP)
// #endif

// #define AX_TXCOE_CTL			0x35
// 	#define AX_TXCOE_IP			0x01
// 	#define AX_TXCOE_TCP			0x02
// 	#define AX_TXCOE_UDP			0x04
// 	#define AX_TXCOE_ICMP			0x08
// 	#define AX_TXCOE_IGMP			0x10
// 	#define AX_TXCOE_TCPV6			0x20
// 	#define AX_TXCOE_UDPV6			0x40
// 	#define AX_TXCOE_ICMV6			0x80
// #if LINUX_VERSION_CODE > KERNEL_VERSION(2, 6, 22)
// 	#define AX_TXCOE_DEF_CSUM	(AX_TXCOE_TCP   | AX_TXCOE_UDP | \
// 					 AX_TXCOE_TCPV6 | AX_TXCOE_UDPV6)
// #else
// 	#define AX_TXCOE_DEF_CSUM	(AX_TXCOE_TCP	| AX_TXCOE_UDP)
// #endif

// #define AX_PAUSE_WATERLVL_HIGH		0x54
// #define AX_PAUSE_WATERLVL_LOW		0x55

// #define AX_EEP_EFUSE_CORRECT		0x00
// #define AX88179_EEPROM_MAGIC			0x17900b95

// /*****************************************************************************/
// /* GMII register definitions */
// #define GMII_PHY_CONTROL			0x00	/* control reg */
// 	/* Bit definitions: GMII Control */
// 	#define GMII_CONTROL_RESET		0x8000	/* reset bit in control reg */
// 	#define GMII_CONTROL_LOOPBACK		0x4000	/* loopback bit in control reg */
// 	#define GMII_CONTROL_10MB		0x0000	/* 10 Mbit */
// 	#define GMII_CONTROL_100MB		0x2000	/* 100Mbit */
// 	#define GMII_CONTROL_1000MB		0x0040	/* 1000Mbit */
// 	#define GMII_CONTROL_SPEED_BITS		0x2040	/* speed bit mask */
// 	#define GMII_CONTROL_ENABLE_AUTO	0x1000	/* autonegotiate enable */
// 	#define GMII_CONTROL_POWER_DOWN		0x0800
// 	#define GMII_CONTROL_ISOLATE		0x0400	/* islolate bit */
// 	#define GMII_CONTROL_START_AUTO		0x0200	/* restart autonegotiate */
// 	#define GMII_CONTROL_FULL_DUPLEX	0x0100

// #define GMII_PHY_STATUS				0x01	/* status reg */
// 	/* Bit definitions: GMII Status */
// 	#define GMII_STATUS_100MB_MASK		0xE000	/* any of these indicate 100 Mbit */
// 	#define GMII_STATUS_10MB_MASK		0x1800	/* either of these indicate 10 Mbit */
// 	#define GMII_STATUS_AUTO_DONE		0x0020	/* auto negotiation complete */
// 	#define GMII_STATUS_AUTO		0x0008	/* auto negotiation is available */
// 	#define GMII_STATUS_LINK_UP		0x0004	/* link status bit */
// 	#define GMII_STATUS_EXTENDED		0x0001	/* extended regs exist */
// 	#define GMII_STATUS_100T4		0x8000	/* capable of 100BT4 */
// 	#define GMII_STATUS_100TXFD		0x4000	/* capable of 100BTX full duplex */
// 	#define GMII_STATUS_100TX		0x2000	/* capable of 100BTX */
// 	#define GMII_STATUS_10TFD		0x1000	/* capable of 10BT full duplex */
// 	#define GMII_STATUS_10T			0x0800	/* capable of 10BT */
// #define GMII_PHY_OUI				0x02	/* most of the OUI bits */
// #define GMII_PHY_MODEL				0x03	/* model/rev bits, and rest of OUI */
// #define GMII_PHY_ANAR				0x04	/* AN advertisement reg */
// 	/* Bit definitions: Auto-Negotiation Advertisement */
// 	#define GMII_ANAR_ASYM_PAUSE		0x0800	/* support asymetric pause */
// 	#define GMII_ANAR_PAUSE			0x0400	/* support pause packets */
// 	#define GMII_ANAR_100T4			0x0200	/* support 100BT4 */
// 	#define GMII_ANAR_100TXFD		0x0100	/* support 100BTX full duplex */
// 	#define GMII_ANAR_100TX			0x0080	/* support 100BTX half duplex */
// 	#define GMII_ANAR_10TFD			0x0040	/* support 10BT full duplex */
// 	#define GMII_ANAR_10T			0x0020	/* support 10BT half duplex */
// 	#define GMII_SELECTOR_FIELD		0x001F	/* selector field. */
// #define GMII_PHY_ANLPAR				0x05	/* AN Link Partner */
// 	/* Bit definitions: Auto-Negotiation Link Partner Ability */
// 	#define GMII_ANLPAR_100T4		0x0200	/* support 100BT4 */
// 	#define GMII_ANLPAR_100TXFD		0x0100	/* support 100BTX full duplex */
// 	#define GMII_ANLPAR_100TX		0x0080	/* support 100BTX half duplex */
// 	#define GMII_ANLPAR_10TFD		0x0040	/* support 10BT full duplex */
// 	#define GMII_ANLPAR_10T			0x0020	/* support 10BT half duplex */
// 	#define GMII_ANLPAR_PAUSE		0x0400	/* support pause packets */
// 	#define GMII_ANLPAR_ASYM_PAUSE		0x0800	/* support asymetric pause */
// 	#define GMII_ANLPAR_ACK			0x4000	/* means LCB was successfully rx'd */
// 	#define GMII_SELECTOR_8023		0x0001;

// #define GMII_PHY_ANER				0x06	/* AN expansion reg */
// #define GMII_PHY_1000BT_CONTROL			0x09	/* control reg for 1000BT */
// #define GMII_PHY_1000BT_STATUS			0x0A	/* status reg for 1000BT */
// #define GMII_PHY_MACR				0x0D
// #define GMII_PHY_MAADR				0x0E

// #define GMII_PHY_PHYSR				0x11	/* PHY specific status register */
// 	#define GMII_PHY_PHYSR_SMASK		0xc000
// 	#define GMII_PHY_PHYSR_GIGA		0x8000
// 	#define GMII_PHY_PHYSR_100		0x4000
// 	#define GMII_PHY_PHYSR_FULL		0x2000
// 	#define GMII_PHY_PHYSR_LINK		0x400

// /* Bit definitions: 1000BaseT AUX Control */
// #define GMII_1000_AUX_CTRL_MASTER_SLAVE		0x1000
// #define GMII_1000_AUX_CTRL_FD_CAPABLE		0x0200	/* full duplex capable */
// #define GMII_1000_AUX_CTRL_HD_CAPABLE		0x0100	/* half duplex capable */
// /* Bit definitions: 1000BaseT AUX Status */
// #define GMII_1000_AUX_STATUS_FD_CAPABLE		0x0800	/* full duplex capable */
// #define GMII_1000_AUX_STATUS_HD_CAPABLE		0x0400	/* half duplex capable */
// /*Cicada MII Registers */
// #define GMII_AUX_CTRL_STATUS			0x1C
// #define GMII_AUX_ANEG_CPLT			0x8000
// #define GMII_AUX_FDX				0x0020
// #define GMII_AUX_SPEED_1000			0x0010
// #define GMII_AUX_SPEED_100			0x0008

// #define GMII_LED_ACTIVE				0x1a
// 	#define GMII_LED_ACTIVE_MASK		0xff8f
// 	#define GMII_LED0_ACTIVE		(1 << 4)
// 	#define GMII_LED1_ACTIVE		(1 << 5)
// 	#define GMII_LED2_ACTIVE		(1 << 6)

// #define GMII_LED_LINK				0x1c
// 	#define GMII_LED_LINK_MASK		0xf888
// 	#define GMII_LED0_LINK_10		(1 << 0)
// 	#define GMII_LED0_LINK_100		(1 << 1)
// 	#define GMII_LED0_LINK_1000		(1 << 2)
// 	#define GMII_LED1_LINK_10		(1 << 4)
// 	#define GMII_LED1_LINK_100		(1 << 5)
// 	#define GMII_LED1_LINK_1000		(1 << 6)
// 	#define GMII_LED2_LINK_10		(1 << 8)
// 	#define GMII_LED2_LINK_100		(1 << 9)
// 	#define GMII_LED2_LINK_1000		(1 << 10)

// 	#define	LED_VALID	(1 << 15) /* UA2 LED Setting */
// 	#define	LED0_ACTIVE	(1 << 0)
// 	#define	LED0_LINK_10	(1 << 1)
// 	#define	LED0_LINK_100	(1 << 2)
// 	#define	LED0_LINK_1000	(1 << 3)
// 	#define	LED0_FD		(1 << 4)
// 	#define LED0_USB3_MASK	0x001f

// 	#define	LED1_ACTIVE	(1 << 5)
// 	#define	LED1_LINK_10	(1 << 6)
// 	#define	LED1_LINK_100	(1 << 7)
// 	#define	LED1_LINK_1000	(1 << 8)
// 	#define	LED1_FD		(1 << 9)
// 	#define LED1_USB3_MASK	0x03e0

// 	#define	LED2_ACTIVE	(1 << 10)
// 	#define	LED2_LINK_1000	(1 << 13)
// 	#define	LED2_LINK_100	(1 << 12)
// 	#define	LED2_LINK_10	(1 << 11)
// 	#define	LED2_FD		(1 << 14)
// 	#define LED2_USB3_MASK	0x7c00

// #define GMII_PHYPAGE				0x1e

// #define GMII_PHY_PAGE_SELECT			0x1f
// 	#define GMII_PHY_PAGE_SELECT_EXT	0x0007
// 	#define GMII_PHY_PAGE_SELECT_PAGE0	0X0000
// 	#define GMII_PHY_PAGE_SELECT_PAGE1	0X0001
// 	#define GMII_PHY_PAGE_SELECT_PAGE2	0X0002
// 	#define GMII_PHY_PAGE_SELECT_PAGE3	0X0003
// 	#define GMII_PHY_PAGE_SELECT_PAGE4	0X0004
// 	#define GMII_PHY_PAGE_SELECT_PAGE5	0X0005
// 	#define GMII_PHY_PAGE_SELECT_PAGE6	0X0006

// /******************************************************************************/
#[allow(non_camel_case_types)]
#[derive(Debug)]
struct ax88179_data {
    rxctl: u16,
    checksum: u8,
    reg_monitor: c_uchar,
}

// struct ax88179_async_handle {
//   	struct usb_ctrlrequest *req;
//   	u8 m_filter[8];
//   	u16 rxctl;
// } __attribute__ ((packed));

// struct ax88179_int_data {
// 	__le16 res1;
// #define AX_INT_PPLS_LINK	(1 << 0)
// #define AX_INT_SPLS_LINK	(1 << 1)
// #define AX_INT_CABOFF_UNPLUG	(1 << 7)
// 	u8 link;
// 	__le16 res2;
// 	u8 status;
// 	__le16 res3;
// } __attribute__ ((packed));

// #define AX_RXHDR_L4_ERR		(1 << 8)
// #define AX_RXHDR_L3_ERR		(1 << 9)

// #define AX_RXHDR_L4_TYPE_ICMP		2
// #define AX_RXHDR_L4_TYPE_IGMP		3
// #define AX_RXHDR_L4_TYPE_TCMPV6		5

// #define AX_RXHDR_L3_TYPE_IP		1
// #define AX_RXHDR_L3_TYPE_IPV6		2

// #define AX_RXHDR_L4_TYPE_MASK			0x1c
// #define AX_RXHDR_L4_TYPE_UDP			4
// #define AX_RXHDR_L4_TYPE_TCP			16
// #define AX_RXHDR_L3CSUM_ERR			2
// #define AX_RXHDR_L4CSUM_ERR			1
// #define AX_RXHDR_CRC_ERR			0x20000000
// #define AX_RXHDR_MII_ERR			0x40000000
// #define AX_RXHDR_DROP_ERR			0x80000000
// #if 0
// struct ax88179_rx_pkt_header {

// 	u8	l4_csum_err:1,
// 		l3_csum_err:1,
// 		l4_type:3,
// 		l3_type:2,
// 		ce:1;

// 	u8	vlan_ind:3,
// 		rx_ok:1,
// 		pri:3,
// 		bmc:1;

// 	u16	len:13,
// 		crc:1,
// 		mii:1,
// 		drop:1;

// } __attribute__ ((packed));
// #endif
// static struct {unsigned char ctrl, timer_l, timer_h, size, ifg; }
// AX88179_BULKIN_SIZE[] =	{
// 	{7, 0x4f, 0,	0x12, 0xff},
// 	{7, 0x20, 3,	0x16, 0xff},
// 	{7, 0xae, 7,	0x18, 0xff},
// 	{7, 0xcc, 0x4c, 0x18, 8},
// };

// static int ax88179_reset(struct usbnet *dev);
// static int ax88179_link_reset(struct usbnet *dev);
// static int ax88179_AutoDetach(struct usbnet *dev, int in_pm);

// static char version[] =
// KERN_INFO "ASIX USB Ethernet Adapter:v" DRIVER_VERSION
// //	" " __TIME__ " " __DATE__ "\n"
// "		http://www.asix.com.tw\n";

// static int msg_enable;
// module_param(msg_enable, int, 0);
// MODULE_PARM_DESC(msg_enable, "usbnet msg_enable");

// static int bsize = -1;
// module_param(bsize, int, 0);
// MODULE_PARM_DESC(bsize, "RX Bulk IN Queue Size");

// static int ifg = -1;
// module_param(ifg, int, 0);
// MODULE_PARM_DESC(ifg, "RX Bulk IN Inter Frame Gap");

// /* EEE advertisement is disabled in default setting */
// static int bEEE = 0;
// module_param(bEEE, int, 0);
// MODULE_PARM_DESC(bEEE, "EEE advertisement configuration");

// /* Green ethernet advertisement is disabled in default setting */
// static int bGETH = 0;
// module_param(bGETH, int, 0);
// MODULE_PARM_DESC(bGETH, "Green ethernet configuration");

/* ASIX AX88179/178A based USB 3.0/2.0 Gigabit Ethernet Devices */
unsafe fn __ax88179_read_cmd(dev: *mut usbnet, cmd: u8, value: u16, index: u16, size: u16, data: *mut c_void, in_pm: c_int) -> KernelResult<()>
{
    assert!(!dev.is_null());

	let f = if in_pm == 0 {
		usbnet_read_cmd
    } else {
        usbnet_read_cmd_nopm
    };

	let ret = f(dev, cmd, (USB_DIR_IN | USB_TYPE_VENDOR | USB_RECIP_DEVICE) as u8, value, index, data, size);

	if unlikely(ret < 0) {
        // netdev_warn(dev->net, "Failed to read reg index 0x%04x: %d\n", index, ret);
        println!("WARNING: ax88179 - failed to read reg index {index:#x}: {ret}");
    }

	KernelResult::from_kernel_errno(ret)
}

unsafe fn __ax88179_write_cmd(
    dev: *mut usbnet,
    cmd: u8,
    value: u16,
    index: u16,
    size: u16,
    data: *mut c_void,
    in_pm: c_int,
) -> KernelResult<()> {
    assert!(!dev.is_null());

    let f = if in_pm == 0 {
        usbnet_write_cmd
    } else {
        usbnet_write_cmd_nopm
    };

    let ret = f(
        dev,
        cmd,
        (USB_DIR_OUT | USB_TYPE_VENDOR | USB_RECIP_DEVICE) as u8,
        value,
        index,
        data,
        size,
    );

    if unlikely(ret < 0) {
        // netdev_warn(dev->net, "Failed to write reg index 0x%04x: %d\n", index, ret);
        println!("WARNING: ax88179 - failed to write reg index {index:#x}: {ret}");
    }

    KernelResult::from_kernel_errno(ret)
}

// static int ax88179_read_cmd_nopm(struct usbnet *dev, u8 cmd, u16 value,
// 				 u16 index, u16 size, void *data, int eflag)
// {
// 	int ret;

// 	if (eflag && (2 == size)) {
// 		u16 buf = 0;
// 		ret = __ax88179_read_cmd(dev, cmd, value, index, size, &buf, 1);
// 		le16_to_cpus(&buf);
// 		*((u16 *)data) = buf;
// 	} else if (eflag && (4 == size)) {
// 		u32 buf = 0;
// 		ret = __ax88179_read_cmd(dev, cmd, value, index, size, &buf, 1);
// 		le32_to_cpus(&buf);
// 		*((u32 *)data) = buf;
// 	} else {
// 		ret = __ax88179_read_cmd(dev, cmd, value, index, size, data, 1);
// 	}

// 	return ret;
// }

// static int ax88179_write_cmd_nopm(struct usbnet *dev, u8 cmd, u16 value,
// 				  u16 index, u16 size, void *data)
// {
// 	int ret;

// 	if (2 == size) {
// 		u16 buf = 0;
// 		buf = *((u16 *)data);
// 		cpu_to_le16s(&buf);
// 		ret = __ax88179_write_cmd(dev, cmd, value, index,
// 					  size, &buf, 1);
// 	} else {
// 		ret = __ax88179_write_cmd(dev, cmd, value, index,
// 					  size, data, 1);
// 	}

// 	return ret;
// }

unsafe fn ax88179_read_cmd(dev: *mut usbnet, cmd: u8, value: u16, index: u16, size: u16, data: *mut c_void, eflag: c_int) -> KernelResult<()>
{
	let result;

	if (eflag != 0) && (2 == size) {
		let mut buf = [0u8; 2];
		result = __ax88179_read_cmd(dev, cmd, value, index, size, &mut buf as *mut _ as _, 0);
		*(data as *mut u16) = u16::from_le_bytes(buf);
	} else if (eflag != 0) && (4 == size) {
		let mut buf = [0u8; 4];
		result = __ax88179_read_cmd(dev, cmd, value, index, size, &mut buf as *mut _ as _, 0);
		*(data as *mut u32) = u32::from_le_bytes(buf);
	} else {
		result = __ax88179_read_cmd(dev, cmd, value, index, size, data, 0);
	}

	result
}

trait WriteData {
    type Output: AsMut<[u8]>;
    fn value(self) -> Self::Output;
}

impl WriteData for () {
    type Output = [u8; 0];
    fn value(self) -> [u8; 0] {
        []
    }
}

impl WriteData for u8 {
    type Output = [u8; 1];
    fn value(self) -> [u8; 1] {
        [self]
    }
}

impl WriteData for u16 {
    type Output = [u8; 2];
    fn value(self) -> [u8; 2] {
        // From original port, le conversion was only done for u16?
        self.to_le_bytes()
    }
}

impl WriteData for u32 {
    type Output = [u8; 4];
    fn value(self) -> [u8; 4] {
        self.to_ne_bytes()
    }
}

impl WriteData for &mut [u8] {
    type Output = Self;
    fn value(self) -> Self {
        self
    }
}

unsafe fn ax88179_write_cmd(
    dev: *mut usbnet,
    cmd: u8,
    value: u16,
    index: u16,
    data: impl WriteData,
) -> KernelResult<()> {
    let mut data = data.value();
    let size = data.as_mut().len() as u16;
    __ax88179_write_cmd(dev, cmd, value, index, size, data.as_mut().as_ptr() as _, 0)
}

// #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 20)
// static void ax88179_async_cmd_callback(struct urb *urb, struct pt_regs *regs)
// #else
// static void ax88179_async_cmd_callback(struct urb *urb)
// #endif
// {
// 	struct ax88179_async_handle *asyncdata = (struct ax88179_async_handle *)urb->context;

// 	if (urb->status < 0)
// 		printk(KERN_ERR "ax88179_async_cmd_callback() failed with %d",
// 		       urb->status);

// 	kfree(asyncdata->req);
// 	kfree(asyncdata);
// 	usb_free_urb(urb);

// }

// static void
// ax88179_write_cmd_async(struct usbnet *dev, u8 cmd, u16 value, u16 index,
// 				    u16 size, void *data)
// {
// 	struct usb_ctrlrequest *req = NULL;
// 	int status = 0;
// 	struct urb *urb = NULL;
// 	void *buf = NULL;
// 	struct ax88179_async_handle *asyncdata = NULL;

// 	urb = usb_alloc_urb(0, GFP_ATOMIC);
// 	if (urb == NULL) {
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
// 		netdev_err(dev->net, "Error allocating URB in write_cmd_async!");
// #else
// 		deverr(dev, "Error allocating URB in write_cmd_async!");
// #endif
// 		return;
// 	}

// 	req = kmalloc(sizeof(struct usb_ctrlrequest), GFP_ATOMIC);
// 	if (req == NULL) {
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
// 		netdev_err(dev->net, "Failed to allocate memory for control request");
// #else
// 		deverr(dev, "Failed to allocate memory for control request");
// #endif
// 		usb_free_urb(urb);
// 		return;
// 	}

// 	asyncdata = (struct ax88179_async_handle*)
// 			kmalloc(sizeof(struct ax88179_async_handle), GFP_ATOMIC);
// 	if (asyncdata == NULL) {
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
// 		netdev_err(dev->net, "Failed to allocate memory for async data");
// #else
// 		deverr(dev, "Failed to allocate memory for async data");
// #endif
// 		kfree(req);
// 		usb_free_urb(urb);
// 		return;
// 	}

// 	asyncdata->req = req;

// 	if (size == 2) {
// 		asyncdata->rxctl = *((u16 *)data);
// 		cpu_to_le16s(&asyncdata->rxctl);
// 		buf = &asyncdata->rxctl;
// 	} else {
// 		memcpy(asyncdata->m_filter, data, size);
// 		buf = asyncdata->m_filter;
// 	}

// 	req->bRequestType = USB_DIR_OUT | USB_TYPE_VENDOR | USB_RECIP_DEVICE;
// 	req->bRequest = cmd;
// 	req->wValue = cpu_to_le16(value);
// 	req->wIndex = cpu_to_le16(index);
// 	req->wLength = cpu_to_le16(size);

// 	usb_fill_control_urb(urb, dev->udev,
// 			     usb_sndctrlpipe(dev->udev, 0),
// 			     (void *)req, buf, size,
// 			     ax88179_async_cmd_callback, asyncdata);

// 	status = usb_submit_urb(urb, GFP_ATOMIC);
// 	if (status < 0) {
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
// 		netdev_err(dev->net, "Error submitting the control message: status=%d",
// 			   status);
// #else
// 		deverr(dev, "Error submitting the control message: status=%d",
// 		       status);
// #endif
// 		kfree(req);
// 		kfree(asyncdata);
// 		usb_free_urb(urb);
// 	}
// }

unsafe extern "C" fn ax88179_status(dev: *mut usbnet, urb: *mut urb) {
    // 	struct ax88179_int_data *event = NULL;
    // 	int link = 0;

    // 	if (urb->actual_length < 8)
    // 		return;

    // 	event = urb->transfer_buffer;
    // 	link = event->link & AX_INT_PPLS_LINK;

    // 	if (netif_carrier_ok(dev->net) != link) {
    // 		if (link)
    // 			usbnet_defer_kevent(dev, EVENT_LINK_RESET);
    // 		else
    // 			netif_carrier_off(dev->net);
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 		netdev_info(dev->net, "ax88179_178a - Link status is: %d\n",
    // 			    link);
    // #else
    // 		devinfo(dev, "ax88179_178a - Link status is: %d\n", link);
    // #endif
    // 	}
}

// static int ax88179_mdio_read(struct net_device *netdev, int phy_id, int loc)
// {
// 	struct usbnet *dev = netdev_priv(netdev);
// 	u16 res;
// 	u16 *tmp16;

// 	tmp16 = kmalloc(2, GFP_KERNEL);
// 	if (!tmp16)
// 		return -ENOMEM;

// 	ax88179_read_cmd(dev, AX_ACCESS_PHY, phy_id, (__u16)loc, 2, tmp16, 1);

// 	res = *tmp16;
// 	kfree(tmp16);

// 	return res;
// }

// static void ax88179_mdio_write(struct net_device *netdev, int phy_id, int loc,
// 			       int val)
// {
// 	struct usbnet *dev = netdev_priv(netdev);
// 	u16 *res;
// 	res = kmalloc(2, GFP_KERNEL);
// 	if (!res)
// 		return;
// 	*res = (u16)val;

// 	ax88179_write_cmd(dev, AX_ACCESS_PHY, phy_id, (__u16)loc, 2, res);

// 	kfree(res);
// }

unsafe extern "C" fn ax88179_suspend(intf: *mut usb_interface, message: pm_message_t) -> c_int {
    // 	struct usbnet *dev = usb_get_intfdata(intf);
    // 	struct ax88179_data *ax179_data = (struct ax88179_data *)dev->data;
    // 	u8 wolp[38] = { 0 };
    // 	u16 tmp16;
    // 	u8 tmp8;

    // usbnet_suspend(intf, message);

    // 	/* Disable RX path */
    // 	ax88179_read_cmd_nopm(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
    // 			      2, 2, &tmp16, 1);
    // 	tmp16 &= ~AX_MEDIUM_RECEIVE_EN;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC,  AX_MEDIUM_STATUS_MODE,
    // 			       2, 2, &tmp16);

    // 	/* Force bz */
    // 	ax88179_read_cmd_nopm(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL,
    // 			      2, 2, &tmp16, 1);
    // 	tmp16 |= AX_PHYPWR_RSTCTL_BZ | AX_PHYPWR_RSTCTL_IPRL;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL,
    // 			       2, 2, &tmp16);

    // 	wolp[28] = 0x04;
    // 	wolp[29] = MASK_WAKEUP_EVENT_TIMER;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_WAKEUP, 0x01, 0, 38, wolp);

    // 	/* change clock */
    // 	tmp8 = 0;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC, AX_CLK_SELECT, 1, 1, &tmp8);

    // 	/* Configure RX control register => stop operation */
    // 	tmp16 = AX_RX_CTL_STOP;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC, AX_RX_CTL, 2, 2, &tmp16);

    // 	tmp8 = ax179_data->reg_monitor;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC, AX_MONITOR_MODE, 1, 1, &tmp8);

    return 0;
}

// static void ax88179_EEE_setting(struct usbnet *dev)
// {
// 	u16 tmp16;

// 	if (bEEE) {
// 		// Enable EEE
// 		tmp16 = 0x07;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  GMII_PHY_MACR, 2, &tmp16);

// 		tmp16 = 0x3c;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  GMII_PHY_MAADR, 2, &tmp16);

// 		tmp16 = 0x4007;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  GMII_PHY_MACR, 2, &tmp16);

// 		tmp16 = 0x06;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  GMII_PHY_MAADR, 2, &tmp16);
// 	} else {
// 		// Disable EEE
// 		tmp16 = 0x07;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  GMII_PHY_MACR, 2, &tmp16);

// 		tmp16 = 0x3c;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  GMII_PHY_MAADR, 2, &tmp16);

// 		tmp16 = 0x4007;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  GMII_PHY_MACR, 2, &tmp16);

// 		tmp16 = 0x00;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  GMII_PHY_MAADR, 2, &tmp16);
// 	}
// }

// static void ax88179_Gether_setting(struct usbnet *dev)
// {
// 	u16 tmp16;

// 	if (bGETH) {
// 		// Enable Green Ethernet
// 		tmp16 = 0x03;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  31, 2, &tmp16);

// 		tmp16 = 0x3247;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  25, 2, &tmp16);

// 		tmp16 = 0x05;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  31, 2, &tmp16);

// 		tmp16 = 0x0680;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  1, 2, &tmp16);

// 		tmp16 = 0;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  31, 2, &tmp16);
// 	} else {
// 		// Disable Green Ethernet
// 		tmp16 = 0x03;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  31, 2, &tmp16);

// 		tmp16 = 0x3246;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  25, 2, &tmp16);

// 		tmp16 = 0;
// 		ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 				  31, 2, &tmp16);
// 	}
// }

unsafe extern "C" fn ax88179_resume(intf: *mut usb_interface) -> c_int {
    return 0;

    // 	struct usbnet *dev = usb_get_intfdata(intf);
    // 	u16 tmp16;
    // 	u8 tmp8;

    // 	netif_carrier_off(dev->net);

    // 	/* Power up ethernet PHY */
    // 	tmp16 = 0;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL,
    // 			       2, 2, &tmp16);
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 36)
    // 	usleep_range(1000, 2000);
    // #else
    // 	msleep(1);
    // #endif
    // 	tmp16 = AX_PHYPWR_RSTCTL_IPRL;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL,
    // 			       2, 2, &tmp16);
    // 	msleep(200);

    // 	/* Ethernet PHY Auto Detach*/
    // 	ax88179_AutoDetach(dev, 1);

    // 	/* change clock */
    // 	ax88179_read_cmd_nopm(dev, AX_ACCESS_MAC,  AX_CLK_SELECT,
    // 			      1, 1, &tmp8, 0);
    // 	tmp8 |= AX_CLK_SELECT_ACS | AX_CLK_SELECT_BCS;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC, AX_CLK_SELECT, 1, 1, &tmp8);
    // 	msleep(100);

    // 	/* Configure RX control register => start operation */
    // 	tmp16 = AX_RX_CTL_DROPCRCERR | AX_RX_CTL_START | AX_RX_CTL_AP |
    // 		 AX_RX_CTL_AMALL | AX_RX_CTL_AB;
    // 	if (NET_IP_ALIGN == 0)
    // 		tmp16 |= AX_RX_CTL_IPE;
    // 	ax88179_write_cmd_nopm(dev, AX_ACCESS_MAC, AX_RX_CTL, 2, 2, &tmp16);

    // 	return usbnet_resume(intf);
}

// static void
// ax88179_get_wol(struct net_device *net, struct ethtool_wolinfo *wolinfo)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	struct ax88179_data *ax179_data = (struct ax88179_data *)dev->data;
// 	u8 opt = ax179_data->reg_monitor;

// 	/*if (ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_MONITOR_MODE,
// 			     1, 1, &opt, 0) < 0) {
// 		wolinfo->supported = 0;
// 		wolinfo->wolopts = 0;
// 		return;
// 	}*/
// 	wolinfo->supported = WAKE_PHY | WAKE_MAGIC;

// 	if (opt & AX_MONITOR_MODE_RWLC)
// 		wolinfo->wolopts |= WAKE_PHY;
// 	if (opt & AX_MONITOR_MODE_RWMP)
// 		wolinfo->wolopts |= WAKE_MAGIC;
// }

// static int
// ax88179_set_wol(struct net_device *net, struct ethtool_wolinfo *wolinfo)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	struct ax88179_data *ax179_data = (struct ax88179_data *)dev->data;
// 	u8 opt = 0;

// 	if (wolinfo->wolopts & WAKE_PHY)
// 		opt |= AX_MONITOR_MODE_RWLC;
// 	else
// 		opt &= ~AX_MONITOR_MODE_RWLC;

// 	if (wolinfo->wolopts & WAKE_MAGIC)
// 		opt |= AX_MONITOR_MODE_RWMP;
// 	else
// 		opt &= ~AX_MONITOR_MODE_RWMP;

// 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MONITOR_MODE, 1, 1, &opt);

// 	ax179_data->reg_monitor = opt;

// 	return 0;
// }

// static int ax88179_get_eeprom_len(struct net_device *net)
// {
// 	return AX_EEPROM_LEN;
// }

// static int
// ax88179_get_eeprom(struct net_device *net, struct ethtool_eeprom *eeprom,
// 		   u8 *data)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	u16 *eeprom_buff = NULL;
// 	int first_word = 0, last_word = 0;
// 	int i = 0;

// 	if (eeprom->len == 0)
// 		return -EINVAL;

// 	eeprom->magic = AX88179_EEPROM_MAGIC;

// 	first_word = eeprom->offset >> 1;
// 	last_word = (eeprom->offset + eeprom->len - 1) >> 1;
// 	eeprom_buff = kmalloc(sizeof(u16) * (last_word - first_word + 1),
// 			      GFP_KERNEL);
// 	if (!eeprom_buff)
// 		return -ENOMEM;

// 	/* ax88179/178A returns 2 bytes from eeprom on read */
// 	for (i = first_word; i <= last_word; i++) {
// 		if (ax88179_read_cmd(dev, AX_ACCESS_EEPROM, i, 1, 2,
// 				     &(eeprom_buff[i - first_word]), 0) < 0) {
// 			kfree(eeprom_buff);
// 			return -EIO;
// 		}
// 	}

// 	memcpy(data, (u8 *)eeprom_buff + (eeprom->offset & 1), eeprom->len);
// 	kfree(eeprom_buff);
// 	return 0;
// }

// static void ax88179_get_drvinfo(struct net_device *net,
// 				struct ethtool_drvinfo *info)
// {
// 	/* Inherit standard device info */
// 	usbnet_get_drvinfo(net, info);
// 	strlcpy (info->version, DRIVER_VERSION, sizeof info->version);
// 	info->eedump_len = 0x3e;
// }
// #if LINUX_VERSION_CODE < KERNEL_VERSION(4, 12, 0)
// static int ax88179_get_settings(struct net_device *net, struct ethtool_cmd *cmd)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	return mii_ethtool_gset(&dev->mii, cmd);
// }

// static int ax88179_set_settings(struct net_device *net, struct ethtool_cmd *cmd)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	return mii_ethtool_sset(&dev->mii, cmd);
// }
// #else
// static
// int ax88179_get_link_ksettings(struct net_device *netdev,
// 			       struct ethtool_link_ksettings *cmd)
// {
// 	struct usbnet *dev = netdev_priv(netdev);

// 	if (!dev->mii.mdio_read)
// 		return -EOPNOTSUPP;

// 	mii_ethtool_get_link_ksettings(&dev->mii, cmd);

// 	return 0;
// }

// static int ax88179_set_link_ksettings(struct net_device *netdev,
// 				      const struct ethtool_link_ksettings *cmd)
// {
// 	struct usbnet *dev = netdev_priv(netdev);

// 	if (!dev->mii.mdio_write)
// 		return -EOPNOTSUPP;

// 	return mii_ethtool_set_link_ksettings(&dev->mii, cmd);
// }
// #endif

// static int ax88179_ioctl(struct net_device *net, struct ifreq *rq, int cmd)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	return  generic_mii_ioctl(&dev->mii, if_mii(rq), cmd, NULL);
// }

// #if LINUX_VERSION_CODE <= KERNEL_VERSION(2, 6, 28)
// static int ax88179_netdev_stop(struct net_device *net)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	u16 *tmp16;

// 	tmp16 = kmalloc(2, GFP_KERNEL);
// 	if (!tmp16)
// 		return -ENOMEM;

// 	ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
// 			 2, 2, tmp16, 1);
// 	*tmp16 &= ~AX_MEDIUM_RECEIVE_EN;
// 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
// 			  2, 2, tmp16);

// 	kfree(tmp16);

// 	return 0;
// }
// #endif

// #if LINUX_VERSION_CODE < KERNEL_VERSION(3, 3, 0)
// static int ax88179_set_csums(struct usbnet *dev)
// {
// 	struct ax88179_data *ax179_data = (struct ax88179_data *)dev->data;
// 	u8* checksum = 0;

// 	checksum = kmalloc(1, GFP_KERNEL);
// 	if (!checksum)
// 		return -ENOMEM;

// 	if (ax179_data->checksum & AX_RX_CHECKSUM)
// 		*checksum = AX_RXCOE_DEF_CSUM;
// 	else
// 		*checksum = 0;

// 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RXCOE_CTL, 1, 1, checksum);

// 	if (ax179_data->checksum & AX_TX_CHECKSUM)
// 		*checksum = AX_TXCOE_DEF_CSUM;
// 	else
// 		*checksum = 0;

// 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_TXCOE_CTL, 1, 1, checksum);

// 	kfree(checksum);

// 	return 0;
// }

// static u32 ax88179_get_tx_csum(struct net_device *netdev)
// {
// 	struct usbnet *dev = netdev_priv(netdev);
// 	struct ax88179_data *ax179_data = (struct ax88179_data *)dev->data;
// 	return ax179_data->checksum & AX_TX_CHECKSUM;
// }

// static u32 ax88179_get_rx_csum(struct net_device *netdev)
// {
// 	struct usbnet *dev = netdev_priv(netdev);
// 	struct ax88179_data *ax179_data = (struct ax88179_data *)dev->data;
// 	return ax179_data->checksum & AX_RX_CHECKSUM;
// }

// static int ax88179_set_rx_csum(struct net_device *netdev, u32 val)
// {
// 	struct usbnet *dev = netdev_priv(netdev);
// 	struct ax88179_data *ax179_data = (struct ax88179_data *)dev->data;

// 	if (val)
// 		ax179_data->checksum |= AX_RX_CHECKSUM;
// 	else
// 		ax179_data->checksum &= ~AX_RX_CHECKSUM;
// 	return ax88179_set_csums(dev);
// }

// static int ax88179_set_tx_csum(struct net_device *netdev, u32 val)
// {
// 	struct usbnet *dev = netdev_priv(netdev);
// 	struct ax88179_data *ax179_data = (struct ax88179_data *)dev->data;

// 	if (val)
// 		ax179_data->checksum |= AX_TX_CHECKSUM;
// 	else
// 		ax179_data->checksum &= ~AX_TX_CHECKSUM;

// 	ethtool_op_set_tx_csum(netdev, val);

// 	return ax88179_set_csums(dev);
// }

// static int ax88179_set_tso(struct net_device *netdev, u32 data)
// {
// 	if (data)
// 		netdev->features |= NETIF_F_TSO;
// 	else
// 		netdev->features &= ~NETIF_F_TSO;

// 	return 0;
// }
// #endif

// static struct ethtool_ops ax88179_ethtool_ops = {
// 	.get_drvinfo		= ax88179_get_drvinfo,
// 	.get_link		= ethtool_op_get_link,
// 	.get_msglevel		= usbnet_get_msglevel,
// 	.set_msglevel		= usbnet_set_msglevel,
// 	.get_wol		= ax88179_get_wol,
// 	.set_wol		= ax88179_set_wol,
// 	.get_eeprom_len		= ax88179_get_eeprom_len,
// 	.get_eeprom		= ax88179_get_eeprom,
// #if LINUX_VERSION_CODE < KERNEL_VERSION(4, 12, 0)
// 	.get_settings		= ax88179_get_settings,
// 	.set_settings		= ax88179_set_settings,
// #else
// 	.get_link_ksettings	= ax88179_get_link_ksettings,
// 	.set_link_ksettings	= ax88179_set_link_ksettings,
// #endif
// #if LINUX_VERSION_CODE < KERNEL_VERSION(3, 3, 0)
// 	.set_tx_csum		= ax88179_set_tx_csum,
// 	.get_tx_csum		= ax88179_get_tx_csum,
// 	.get_rx_csum		= ax88179_get_rx_csum,
// 	.set_rx_csum		= ax88179_set_rx_csum,
// 	.get_tso		= ethtool_op_get_tso,
// 	.set_tso		= ax88179_set_tso,
// 	.get_sg			= ethtool_op_get_sg,
// 	.set_sg			= ethtool_op_set_sg
// #endif
// };

// static void ax88179_set_multicast(struct net_device *net)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	struct ax88179_data *data = (struct ax88179_data *)&dev->data;
// 	u8 *m_filter = ((u8 *)dev->data) + 12;
// 	int mc_count = 0;

// #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 35)
// 	mc_count = net->mc_count;
// #else
// 	mc_count = netdev_mc_count(net);
// #endif

// 	data->rxctl = (AX_RX_CTL_START | AX_RX_CTL_AB);
// 	if (NET_IP_ALIGN == 0)
// 		data->rxctl |= AX_RX_CTL_IPE;

// 	if (net->flags & IFF_PROMISC) {
// 		data->rxctl |= AX_RX_CTL_PRO;
// 	} else if (net->flags & IFF_ALLMULTI
// 		   || mc_count > AX_MAX_MCAST) {
// 		data->rxctl |= AX_RX_CTL_AMALL;
// 	} else if (mc_count == 0) {
// 		/* just broadcast and directed */
// 	} else {
// 		/* We use the 20 byte dev->data
// 		 * for our 8 byte filter buffer
// 		 * to avoid allocating memory that
// 		 * is tricky to free later */
// 		u32 crc_bits = 0;

// #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 35)
// 		struct dev_mc_list *mc_list = net->mc_list;
// 		int i = 0;

// 		memset(m_filter, 0, AX_MCAST_FILTER_SIZE);

// 		/* Build the multicast hash filter. */
// 		for (i = 0; i < net->mc_count; i++) {
// 			crc_bits =
// 			    ether_crc(ETH_ALEN,
// 				      mc_list->dmi_addr) >> 26;
// 			*(m_filter + (crc_bits >> 3)) |=
// 				1 << (crc_bits & 7);
// 			mc_list = mc_list->next;
// 		}
// #else
// 		struct netdev_hw_addr *ha = NULL;
// 		memset(m_filter, 0, AX_MCAST_FILTER_SIZE);
// 		netdev_for_each_mc_addr(ha, net) {
// 			crc_bits = ether_crc(ETH_ALEN, ha->addr) >> 26;
// 			*(m_filter + (crc_bits >> 3)) |=
// 				1 << (crc_bits & 7);
// 		}
// #endif
// 		ax88179_write_cmd_async(dev, AX_ACCESS_MAC,
// 					AX_MULTI_FILTER_ARRY,
// 					AX_MCAST_FILTER_SIZE,
// 					AX_MCAST_FILTER_SIZE, m_filter);

// 		data->rxctl |= AX_RX_CTL_AM;
// 	}

// 	ax88179_write_cmd_async(dev, AX_ACCESS_MAC, AX_RX_CTL,
// 				2, 2, &data->rxctl);
// }

// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 39)
// static int
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(3, 3, 0)
// ax88179_set_features(struct net_device *net, netdev_features_t features)
// #else
// ax88179_set_features(struct net_device *net, u32 features)
// #endif

// {
// 	u8 *tmp8;
// 	struct usbnet *dev = netdev_priv(net);

// #if LINUX_VERSION_CODE >= KERNEL_VERSION(3, 3, 0)
// 	netdev_features_t changed = net->features ^ features;
// #else
// 	u32 changed = net->features ^ features;
// #endif

// 	tmp8 = kmalloc(1, GFP_KERNEL);
// 	if (!tmp8)
// 		return -ENOMEM;

// 	if (changed & NETIF_F_IP_CSUM) {
// 		ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_TXCOE_CTL,
// 				 1, 1, tmp8, 0);
// 		*tmp8 ^= AX_TXCOE_TCP | AX_TXCOE_UDP;
// 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_TXCOE_CTL, 1, 1, tmp8);
// 	}

// 	if (changed & NETIF_F_IPV6_CSUM) {
// 		ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_TXCOE_CTL,
// 				 1, 1, tmp8, 0);
// 		*tmp8 ^= AX_TXCOE_TCPV6 | AX_TXCOE_UDPV6;
// 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_TXCOE_CTL, 1, 1, tmp8);
// 	}

// 	if (changed & NETIF_F_RXCSUM) {
// 		ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_RXCOE_CTL,
// 				 1, 1, tmp8, 0);
// 		*tmp8 ^= AX_RXCOE_IP | AX_RXCOE_TCP | AX_RXCOE_UDP |
// 		       AX_RXCOE_TCPV6 | AX_RXCOE_UDPV6;
// 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RXCOE_CTL, 1, 1, tmp8);
// 	}

// 	kfree(tmp8);

// 	return 0;
// }
// #endif

// static int ax88179_change_mtu(struct net_device *net, int new_mtu)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	u16 *tmp16;

// 	if (new_mtu <= 0 || new_mtu > 4088)
// 		return -EINVAL;

// 	net->mtu = new_mtu;
// 	dev->hard_mtu = net->mtu + net->hard_header_len;

// 	tmp16 = kmalloc(2, GFP_KERNEL);
// 	if (!tmp16)
// 		return -ENOMEM;

// 	if (net->mtu > 1500) {
// 		ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
// 				 2, 2, tmp16, 1);
// 		*tmp16 |= AX_MEDIUM_JUMBO_EN;
// 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
// 				  2, 2, tmp16);
// 	} else {
// 		ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
// 				 2, 2, tmp16, 1);
// 		*tmp16 &= ~AX_MEDIUM_JUMBO_EN;
// 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
// 				  2, 2, tmp16);
// 	}

// #if LINUX_VERSION_CODE >= KERNEL_VERSION(3, 12, 0)
// 	usbnet_update_max_qlen(dev);
// #endif

// 	kfree(tmp16);

// 	return 0;
// }

// static int ax88179_set_mac_addr(struct net_device *net, void *p)
// {
// 	struct usbnet *dev = netdev_priv(net);
// 	struct sockaddr *addr = p;
// 	int ret;

// 	if (netif_running(net))
// 		return -EBUSY;
// 	if (!is_valid_ether_addr(addr->sa_data))
// 		return -EADDRNOTAVAIL;

// 	memcpy(net->dev_addr, addr->sa_data, ETH_ALEN);

// 	/* Set the MAC address */
// 	ret =  ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_NODE_ID, ETH_ALEN,
// 				 ETH_ALEN, net->dev_addr);
// 	if (ret < 0)
// 		return ret;

// 	return 0;
// }

// #if LINUX_VERSION_CODE > KERNEL_VERSION(2, 6, 29)
// static const struct net_device_ops ax88179_netdev_ops = {
// 	.ndo_open		= usbnet_open,
// 	.ndo_stop		= usbnet_stop,
// 	.ndo_start_xmit		= usbnet_start_xmit,
// 	.ndo_tx_timeout		= usbnet_tx_timeout,
// 	.ndo_change_mtu		= ax88179_change_mtu,
// 	.ndo_do_ioctl		= ax88179_ioctl,
// 	.ndo_set_mac_address	= ax88179_set_mac_addr,
// 	.ndo_validate_addr	= eth_validate_addr,
// #if LINUX_VERSION_CODE <= KERNEL_VERSION(3, 2, 0)
// 	.ndo_set_multicast_list	= ax88179_set_multicast,
// #else
// 	.ndo_set_rx_mode	= ax88179_set_multicast,
// #endif
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 39)
// 	.ndo_set_features	= ax88179_set_features,
// #endif
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(4, 12, 0)
// 	.ndo_get_stats64	= usbnet_get_stats64,
// #endif
// };
// #endif

// static int ax88179_check_eeprom(struct usbnet *dev)
// {
// 	u8 i = 0;
// 	u8 *buf;
// 	u8 *eeprom;
// 	u16 csum = 0, delay = HZ / 10;
// 	unsigned long jtimeout = 0;

// 	eeprom = kmalloc(22, GFP_KERNEL);
// 	if (!eeprom)
// 		return -ENOMEM;
// 	buf = &eeprom[20];

// 	/* Read EEPROM content */
// 	for (i = 0 ; i < 6; i++) {
// 		buf[0] = i;
// 		if (ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_SROM_ADDR,
// 				      1, 1, buf) < 0) {
// 			kfree(eeprom);
// 			return -EINVAL;
// 		}

// 		buf[0] = EEP_RD;
// 		if (ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_SROM_CMD,
// 				      1, 1, buf) < 0) {
// 			kfree(eeprom);
// 			return -EINVAL;
// 		}

// 		jtimeout = jiffies + delay;
// 		do {
// 			ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_SROM_CMD,
// 					 1, 1, buf, 0);

// 			if (time_after(jiffies, jtimeout)) {
// 				kfree(eeprom);
// 				return -EINVAL;
// 			}
// 		} while (buf[0] & EEP_BUSY);

// 		ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_SROM_DATA_LOW,
// 				 2, 2, &eeprom[i * 2], 0);

// 		if ((i == 0) && (eeprom[0] == 0xFF)) {
// 			kfree(eeprom);
// 			return -EINVAL;
// 		}
// 	}

// 	csum = eeprom[6] + eeprom[7] + eeprom[8] + eeprom[9];
// 	csum = (csum >> 8) + (csum & 0xff);

// 	if ((csum + eeprom[10]) == 0xff) {
// 		kfree(eeprom);
// 		return AX_EEP_EFUSE_CORRECT;
// 	} else {
// 		kfree(eeprom);
// 		return -EINVAL;
// 	}
// }

// static int ax88179_check_efuse(struct usbnet *dev, void *ledmode)
// {
// 	u8	i = 0;
// 	u16	csum = 0;
// 	u8	*efuse;

// 	efuse = kmalloc(64, GFP_KERNEL);
// 	if (!efuse)
// 		return -ENOMEM;

// 	if (ax88179_read_cmd(dev, AX_ACCESS_EFUSE, 0, 64, 64, efuse, 0) < 0) {
// 		kfree(efuse);
// 		return -EINVAL;
// 	}

// 	if (efuse[0] == 0xFF) {
// 		kfree(efuse);
// 		return -EINVAL;
// 	}

// 	for (i = 0; i < 64; i++)
// 		csum = csum + efuse[i];

// 	while (csum > 255)
// 		csum = (csum & 0x00FF) + ((csum >> 8) & 0x00FF);

// 	if (csum == 0xFF) {
// 		memcpy((u8 *)ledmode, &efuse[51], 2);
// 		kfree(efuse);
// 		return AX_EEP_EFUSE_CORRECT;
// 	} else {
// 		kfree(efuse);
// 		return -EINVAL;
// 	}
// }

// static int ax88179_convert_old_led(struct usbnet *dev, u8 efuse, void *ledvalue)
// {
// 	u8 ledmode = 0;
// 	u16 *tmp16;
// 	u16 led = 0;

// 	tmp16 = kmalloc(2, GFP_KERNEL);
// 	if (!tmp16)
// 		return -ENOMEM;

// 	/* loaded the old eFuse LED Mode */
// 	if (efuse) {
// 		if (ax88179_read_cmd(dev, AX_ACCESS_EFUSE, 0x18,
// 				     1, 2, tmp16, 1) < 0) {
// 			kfree(tmp16);
// 			return -EINVAL;
// 	}
// 		ledmode = (u8)(*tmp16 & 0xFF);
// 	} else { /* loaded the old EEprom LED Mode */
// 		if (ax88179_read_cmd(dev, AX_ACCESS_EEPROM, 0x3C,
// 				     1, 2, tmp16, 1) < 0) {
// 			kfree(tmp16);
// 			return -EINVAL;
// 		}
// 		ledmode = (u8) (*tmp16 >> 8);
// 	}
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
// 	netdev_dbg(dev->net, "Old LED Mode = %02X\n", ledmode);
// #else
// 	devdbg(dev, "Old LED Mode = %02X\n", ledmode);
// #endif
// 	switch (ledmode) {
// 	case 0xFF:
// 		led = LED0_ACTIVE | LED1_LINK_10 | LED1_LINK_100 |
// 		      LED1_LINK_1000 | LED2_ACTIVE | LED2_LINK_10 |
// 		      LED2_LINK_100 | LED2_LINK_1000 | LED_VALID;
// 		break;
// 	case 0xFE:
// 		led = LED0_ACTIVE | LED1_LINK_1000 | LED2_LINK_100 | LED_VALID;
// 		break;
// 	case 0xFD:
// 		led = LED0_ACTIVE | LED1_LINK_1000 | LED2_LINK_100 |
// 		      LED2_LINK_10 | LED_VALID;
// 		break;
// 	case 0xFC:
// 		led = LED0_ACTIVE | LED1_ACTIVE | LED1_LINK_1000 | LED2_ACTIVE |
// 		      LED2_LINK_100 | LED2_LINK_10 | LED_VALID;
// 		break;
// 	default:
// 		led = LED0_ACTIVE | LED1_LINK_10 | LED1_LINK_100 |
// 		      LED1_LINK_1000 | LED2_ACTIVE | LED2_LINK_10 |
// 		      LED2_LINK_100 | LED2_LINK_1000 | LED_VALID;
// 		break;
// 	}

// 	memcpy((u8 *)ledvalue, &led, 2);
// 	kfree(tmp16);

// 	return 0;
// }

// static int ax88179_led_setting(struct usbnet *dev)
// {

// 	u16 ledvalue = 0, delay = HZ / 10;
// 	u16 *ledact, *ledlink;
// 	u16 *tmp16;
// 	u8 *value;
// 	u8 *tmp;
// 	unsigned long jtimeout = 0;

// 	tmp = kmalloc(6, GFP_KERNEL);
// 	if (!tmp)
// 		return -ENOMEM;

// 	value = (u8*)tmp;
// 	tmp16 = (u16*)tmp;
// 	ledact = (u16*)(&tmp[2]);
// 	ledlink = (u16*)(&tmp[4]);

// 	/* Check AX88179 version. UA1 or UA2 */
// 	ax88179_read_cmd(dev, AX_ACCESS_MAC, GENERAL_STATUS, 1, 1, value, 0);

// 	/* UA1 */
// 	if (!(*value & AX_SECLD)) {
// 		*value = AX_GPIO_CTRL_GPIO3EN | AX_GPIO_CTRL_GPIO2EN |
// 			AX_GPIO_CTRL_GPIO1EN;
// 		if (ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_GPIO_CTRL,
// 				      1, 1, value) < 0) {
// 			kfree(tmp);
// 			return -EINVAL;
// 		}
// 	}

// 	/* check EEprom */
// 	if (ax88179_check_eeprom(dev) == AX_EEP_EFUSE_CORRECT) {
// 		*value = 0x42;
// 		if (ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_SROM_ADDR,
// 				      1, 1, value) < 0) {
// 			kfree(tmp);
// 			return -EINVAL;
// 		}

// 		*value = EEP_RD;
// 		if (ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_SROM_CMD,
// 				      1, 1, value) < 0) {
// 			kfree(tmp);
// 			return -EINVAL;
// 		}

// 		jtimeout = jiffies + delay;
// 		do {
// 			ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_SROM_CMD,
// 					 1, 1, value, 0);

// 			ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_SROM_CMD,
// 					 1, 1, value, 0);

// 			if (time_after(jiffies, jtimeout))
// 				return -EINVAL;
// 		} while (*value & EEP_BUSY);

// 		ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_SROM_DATA_HIGH,
// 				 1, 1, value, 0);
// 		ledvalue = (*value << 8);
// 		ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_SROM_DATA_LOW,
// 				 1, 1, value, 0);
// 		ledvalue |= *value;

// 		/* load internal ROM for defaule setting */
// 		if ((ledvalue == 0xFFFF) || ((ledvalue & LED_VALID) == 0))
// 			ax88179_convert_old_led(dev, 0, &ledvalue);

// 	} else if (ax88179_check_efuse(dev, &ledvalue) ==
// 				       AX_EEP_EFUSE_CORRECT) { /* check efuse */
// 		if ((ledvalue == 0xFFFF) || ((ledvalue & LED_VALID) == 0))
// 			ax88179_convert_old_led(dev, 0, &ledvalue);
// 	} else {
// 		ax88179_convert_old_led(dev, 0, &ledvalue);
// 	}

// 	*tmp16 = GMII_PHY_PAGE_SELECT_EXT;
// 	ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 			  GMII_PHY_PAGE_SELECT, 2, tmp16);

// 	*tmp16 = 0x2c;
// 	ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 			  GMII_PHYPAGE, 2, tmp16);

// 	ax88179_read_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 			 GMII_LED_ACTIVE, 2, ledact, 1);

// 	ax88179_read_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 			 GMII_LED_LINK, 2, ledlink, 1);

// 	*ledact &= GMII_LED_ACTIVE_MASK;
// 	*ledlink &= GMII_LED_LINK_MASK;

// 	if (ledvalue & LED0_ACTIVE)
// 		*ledact |= GMII_LED0_ACTIVE;
// 	if (ledvalue & LED1_ACTIVE)
// 		*ledact |= GMII_LED1_ACTIVE;
// 	if (ledvalue & LED2_ACTIVE)
// 		*ledact |= GMII_LED2_ACTIVE;

// 	if (ledvalue & LED0_LINK_10)
// 		*ledlink |= GMII_LED0_LINK_10;
// 	if (ledvalue & LED1_LINK_10)
// 		*ledlink |= GMII_LED1_LINK_10;
// 	if (ledvalue & LED2_LINK_10)
// 		*ledlink |= GMII_LED2_LINK_10;

// 	if (ledvalue & LED0_LINK_100)
// 		*ledlink |= GMII_LED0_LINK_100;
// 	if (ledvalue & LED1_LINK_100)
// 		*ledlink |= GMII_LED1_LINK_100;
// 	if (ledvalue & LED2_LINK_100)
// 		*ledlink |= GMII_LED2_LINK_100;

// 	if (ledvalue & LED0_LINK_1000)
// 		*ledlink |= GMII_LED0_LINK_1000;
// 	if (ledvalue & LED1_LINK_1000)
// 		*ledlink |= GMII_LED1_LINK_1000;
// 	if (ledvalue & LED2_LINK_1000)
// 		*ledlink |= GMII_LED2_LINK_1000;

// 	ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 			  GMII_LED_ACTIVE, 2, ledact);

// 	ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 			  GMII_LED_LINK, 2, ledlink);

// 	*tmp16 = GMII_PHY_PAGE_SELECT_PAGE0;
// 	ax88179_write_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID,
// 			  GMII_PHY_PAGE_SELECT, 2, tmp16);

// 	/* LED full duplex setting */
// 	*tmp16 = 0;
// 	if (ledvalue & LED0_FD)
// 		*tmp16 |= 0x01;
// 	else if ((ledvalue & LED0_USB3_MASK) == 0)
// 		*tmp16 |= 0x02;

// 	if (ledvalue & LED1_FD)
// 		*tmp16 |= 0x04;
// 	else if ((ledvalue & LED1_USB3_MASK) == 0)
// 		*tmp16 |= 0x08;

// 	if (ledvalue & LED2_FD) /* LED2_FD */
// 		*tmp16 |= 0x10;
// 	else if ((ledvalue & LED2_USB3_MASK) == 0) /* LED2_USB3 */
// 		*tmp16 |= 0x20;

// 	ax88179_write_cmd(dev, AX_ACCESS_MAC, 0x73, 1, 1, tmp16);

// 	kfree(tmp);

// 	return 0;
// }

// static int ax88179_AutoDetach(struct usbnet *dev, int in_pm)
// {
// 	u16 *tmp16;
// 	u8 *tmp8;
// 	int (*fnr)(struct usbnet *, u8, u16, u16, u16, void *, int);
// 	int (*fnw)(struct usbnet *, u8, u16, u16, u16, void *);

// 	if (!in_pm) {
// 		fnr = ax88179_read_cmd;
// 		fnw = ax88179_write_cmd;
// 	} else {
// 		fnr = ax88179_read_cmd_nopm;
// 		fnw = ax88179_write_cmd_nopm;
// 	}

// 	tmp16 = kmalloc(3, GFP_KERNEL);
// 	if (!tmp16)
// 		return -ENOMEM;

// 	tmp8 = (u8*)(&tmp16[2]);

// 	if (fnr(dev, AX_ACCESS_EEPROM, 0x43, 1, 2, tmp16, 1) < 0) {
// 		kfree(tmp16);
// 		return 0;
// 	}

// 	if ((*tmp16 == 0xFFFF) || (!(*tmp16 & 0x0100))) {
// 		kfree(tmp16);
// 		return 0;
// 	}

// 	/* Enable Auto Detach bit */
// 	*tmp8 = 0;
// 	fnr(dev, AX_ACCESS_MAC, AX_CLK_SELECT, 1, 1, tmp8, 0);
// 	*tmp8 |= AX_CLK_SELECT_ULR;
// 	fnw(dev, AX_ACCESS_MAC, AX_CLK_SELECT, 1, 1, tmp8);

// 	fnr(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL, 2, 2, tmp16, 1);
// 	*tmp16 |= AX_PHYPWR_RSTCTL_AUTODETACH;
// 	fnw(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL, 2, 2, tmp16);

// 	kfree(tmp16);

// 	return 0;
// }

unsafe fn access_eeprom_mac(dev: *mut usbnet, buf: *mut u8, offset: u8, wflag: c_int) -> KernelResult<()> {
    let tmp: *mut u16 = buf as *mut u16;

    for i in 0..(ETH_ALEN as u8 >> 1) {
        if wflag > 0 {
            ax88179_write_cmd(dev, AX_ACCESS_EEPROM, (offset + i) as u16, 1, *tmp.offset(i as isize))?;
            // FIXME: mdelay is a macro so using msleep for now
            // mdelay(15);
            msleep(15);
        } else {
            let result = ax88179_read_cmd(dev, AX_ACCESS_EEPROM, (offset + i) as u16, 1, 2, tmp.offset(i as isize) as _, 0);
            if let Err(e) = result {
                println!("DEBUG: ax88179 - failed to read MAC address from EEPROM: {}", e.into_kernel_errno());
                return result;
            }
        }
    }

    if wflag == 0 {
        core::slice::from_raw_parts_mut((*(*dev).net).dev_addr, ETH_ALEN as usize)
            .copy_from_slice(core::slice::from_raw_parts_mut(buf, ETH_ALEN as usize))
    } else {
        /* reload eeprom data */
        ax88179_write_cmd(dev, AX_RELOAD_EEPROM_EFUSE, 0, 0, ())?;
    }

    Ok(())
}

unsafe fn ax88179_check_ether_addr(dev: *mut usbnet) -> c_int
{
	let tmp = core::slice::from_raw_parts_mut((*(*dev).net).dev_addr as *mut u8, ETH_ALEN as usize);
	let default_mac: [u8; 6] = [0, 0x0e, 0xc6, 0x81, 0x79, 0x01];
	let default_mac_178a: [u8; 6] = [0, 0x0e, 0xc6, 0x81, 0x78, 0x01];

    // TODO: missing bindings to is_valid_ether_addr and eth_hw_addr_random

	// if (tmp[0] == 0 && tmp[1] == 0 && tmp[2] == 0) ||
    //     !is_valid_ether_addr(tmp.as_mut_ptr()) ||
    //     tmp == &default_mac ||
    //     tmp == default_mac_178a {
	// 	let i;

    //     // TODO: maybe format address better? See printk calls below.
	// 	println!("Found invalid EEPROM MAC address value: {tmp:x}");

	// 	// for i in 0..ETH_ALEN {
	// 	// 	printk("%02X", *((u8*)tmp + i));
	// 	// 	if (i != 5)
	// 	// 		printk("-");
	// 	// }
	// 	// printk("\n");
	// 	eth_hw_addr_random(*dev.net);

	// 	*tmp = 0;
	// 	*(tmp + 1) = 0x0E;
	// 	*(tmp + 2) = 0xC6;
	// 	*(tmp + 3) = 0x8E;

	// 	return -(EADDRNOTAVAIL as c_int);
	// }
	return 0;
}

unsafe fn ax88179_get_mac(dev: *mut usbnet, buf: *mut u8) -> KernelResult<()> {
    access_eeprom_mac(dev, buf, 0x0, 0)?;

    // TODO: enable
    // if ax88179_check_ether_addr(dev) != 0 {
    //     ret = access_eeprom_mac(dev, (*(*dev).net).dev_addr, 0x0, 1);
    //     if ret < 0 {
    //         // netdev_err(dev->net, "Failed to write MAC to EEPROM: %d", ret);
    //         println!("ERROR: ax88179 - failed to write MAC to EEPROM: {ret}");
    //         return ret;
    //     }

    //     msleep(5);

    //     ret = ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_NODE_ID, ETH_ALEN, ETH_ALEN, buf, 0);
    //     if ret < 0 {
    //         // netdev_err(dev->net, "Failed to read MAC address: %d", ret);
    //         println!("ERROR: ax88179 - failed to read MAC address: {ret}");
    //         return ret;
    //     }

    //     for i in 0..ETH_ALEN {
    //         if *((*(*dev).net).dev_addr.offset(i)) != *(buf.offset(i)) {
    //             // netdev_warn(dev->net, "Found invalid EEPROM part or non-EEPROM");
    //             println!("ERROR: ax88179 - found invalid EEPROM part or non-EEPROM");
    //             break;
    //         }
    //     }
    // }

    (*(*dev).net).perm_addr[..ETH_ALEN as usize]
        .copy_from_slice(core::slice::from_raw_parts_mut((*(*dev).net).dev_addr as *mut u8, ETH_ALEN as usize));

    let result = ax88179_write_cmd(
        dev,
        AX_ACCESS_MAC,
        AX_NODE_ID,
        ETH_ALEN as u16,
        core::slice::from_raw_parts_mut((*(*dev).net).dev_addr as *mut u8, ETH_ALEN as usize),
    );

    if let Err(e) = result {
        // netdev_err(dev->net, "Failed to write MAC address: %d", ret);
        println!("ERROR: ax88179 - failed to write MAC address: {}", e.to_kernel_errno());
    }

    result
}

unsafe fn try_ax88179_bind(dev: *mut usbnet, intf: *mut usb_interface) -> KernelResult<()> {
    println!("ax88179_bind");

    let data: *mut ax88179_data = transmute((&mut *dev).data.as_ptr());

    let mut tmp32: u32;
    let mut tmp16: u16;
    let mut tmp: u8;
    let mut mac = [0u8; ETH_ALEN as usize];
    let mut ret: c_int;

    usbnet_get_endpoints(dev, intf);

    // if (msg_enable != 0)
    // 	dev->msg_enable = msg_enable;

    data.write(zeroed());

    tmp32 = 0;
    ax88179_write_cmd(dev, 0x81, 0x310, 0, tmp32)?;

    /* Power up ethernet PHY */
    tmp16 = 0;
    ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL, 2, tmp16)?;
    tmp16 = AX_PHYPWR_RSTCTL_IPRL;
    ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL, 2, tmp16)?;
    msleep(200);

    tmp = AX_CLK_SELECT_ACS | AX_CLK_SELECT_BCS;
    ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_CLK_SELECT, 1, tmp)?;
    msleep(100);

    /* Get the MAC address */
    ax88179_get_mac(dev, mac.as_mut_ptr())?;

    println!("Got mac value of {mac:?}");
    // 	if (ret)
    // 		goto out;

    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 		netdev_dbg(dev->net, "MAC [%02x-%02x-%02x-%02x-%02x-%02x]\n",
    // 			   dev->net->dev_addr[0], dev->net->dev_addr[1],
    // 			   dev->net->dev_addr[2], dev->net->dev_addr[3],
    // 			   dev->net->dev_addr[4], dev->net->dev_addr[5]);
    // #else
    // 		devdbg(dev, "MAC [%02x-%02x-%02x-%02x-%02x-%02x]\n",
    // 		       dev->net->dev_addr[0], dev->net->dev_addr[1],
    // 		       dev->net->dev_addr[2], dev->net->dev_addr[3],
    // 		       dev->net->dev_addr[4], dev->net->dev_addr[5]);
    // #endif

    // 	/* RX bulk configuration, default for USB3.0 to Giga*/
    // 	memcpy(mac, &AX88179_BULKIN_SIZE[0], 5);
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_BULKIN_QCTRL, 5, 5, mac);

    // 	dev->rx_urb_size = 1024 * 20;

    // 	tmp = 0x34;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PAUSE_WATERLVL_LOW, 1, 1, &tmp);

    // 	tmp = 0x52;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PAUSE_WATERLVL_HIGH,
    // 			  1, 1, &tmp);

    // 	/* Disable auto-power-OFF GigaPHY after ethx down*/
    // 	ax88179_write_cmd(dev, 0x91, 0, 0, 0, NULL);

    // #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 30)
    // 	dev->net->do_ioctl = ax88179_ioctl;
    // 	dev->net->set_multicast_list = ax88179_set_multicast;
    // 	dev->net->set_mac_address = ax88179_set_mac_addr;
    // 	dev->net->change_mtu = ax88179_change_mtu;
    // #if LINUX_VERSION_CODE <= KERNEL_VERSION(2, 6, 28)
    // 	dev->net->stop = ax88179_netdev_stop;
    // #endif
    // #else
    // 	dev->net->netdev_ops = &ax88179_netdev_ops;
    // #endif

    // 	dev->net->ethtool_ops = &ax88179_ethtool_ops;
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 30)
    // 	dev->net->needed_headroom = 8;
    // #endif
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(4, 10, 0)
    // 	dev->net->max_mtu = 4088;
    // #endif

    // 	/* Initialize MII structure */
    // 	dev->mii.dev = dev->net;
    // 	dev->mii.mdio_read = ax88179_mdio_read;
    // 	dev->mii.mdio_write = ax88179_mdio_write;
    // 	dev->mii.phy_id_mask = 0xff;
    // 	dev->mii.reg_num_mask = 0xff;
    // 	dev->mii.phy_id = 0x03;
    // 	dev->mii.supports_gmii = 1;

    // 	dev->net->features |= NETIF_F_IP_CSUM;
    // #if LINUX_VERSION_CODE > KERNEL_VERSION(2, 6, 22)
    // 	dev->net->features |= NETIF_F_IPV6_CSUM;
    // #endif
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(3, 12, 0)
    // 	if (usb_device_no_sg_constraint(dev->udev))
    // 		dev->can_dma_sg = 1;
    // 	dev->net->features |= NETIF_F_SG | NETIF_F_TSO;
    // #endif

    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 39)
    // 	dev->net->hw_features |= NETIF_F_IP_CSUM;
    // 	dev->net->hw_features |= NETIF_F_IPV6_CSUM;
    // 	dev->net->hw_features |= NETIF_F_SG | NETIF_F_TSO;
    // #endif

    // 	/* Enable checksum offload */
    // 	tmp = AX_RXCOE_IP | AX_RXCOE_TCP | AX_RXCOE_UDP |
    // 	      AX_RXCOE_TCPV6 | AX_RXCOE_UDPV6;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RXCOE_CTL, 1, 1, &tmp);

    // 	tmp = AX_TXCOE_IP | AX_TXCOE_TCP | AX_TXCOE_UDP |
    // 	      AX_TXCOE_TCPV6 | AX_TXCOE_UDPV6;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_TXCOE_CTL, 1, 1, &tmp);

    // 	ax179_data->checksum |= AX_RX_CHECKSUM | AX_TX_CHECKSUM;

    // 	/* Configure RX control register => start operation */
    // 	tmp16 = AX_RX_CTL_DROPCRCERR | AX_RX_CTL_START | AX_RX_CTL_AP |
    // 		 AX_RX_CTL_AMALL | AX_RX_CTL_AB;
    // 	if (NET_IP_ALIGN == 0)
    // 		tmp16 |= AX_RX_CTL_IPE;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_CTL, 2, 2, &tmp16);

    // 	tmp = AX_MONITOR_MODE_PMETYPE | AX_MONITOR_MODE_PMEPOL |
    // 	      AX_MONITOR_MODE_RWMP;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MONITOR_MODE, 1, 1, &tmp);

    // 	ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_MONITOR_MODE, 1, 1, &tmp, 0);
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 		netdev_dbg(dev->net, "Monitor mode = 0x%02x\n", tmp);
    // #else
    // 		devdbg(dev, "Monitor mode = 0x%02x\n", tmp);
    // #endif
    // 	/* Configure default medium type => giga */
    // 	tmp16 = AX_MEDIUM_RECEIVE_EN	 | AX_MEDIUM_TXFLOW_CTRLEN |
    // 		AX_MEDIUM_RXFLOW_CTRLEN | AX_MEDIUM_FULL_DUPLEX   |
    // 		AX_MEDIUM_GIGAMODE;

    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
    // 			  2, 2, &tmp16);

    // 	ax88179_led_setting(dev);

    // 	ax88179_EEE_setting(dev);

    // 	ax88179_Gether_setting(dev);

    // 	ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_MONITOR_MODE, 1, 1, &tmp, 0);
    // 	ax179_data->reg_monitor = tmp;

    // 	/* Restart autoneg */
    // 	mii_nway_restart(&dev->mii);

    // 	netif_carrier_off(dev->net);

    // 	printk(version);
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 		netdev_info(dev->net, "mtu %d\n", dev->net->mtu);
    // #else
    // 		devinfo(dev, "mtu %d\n", dev->net->mtu);
    // #endif
    // 	return 0;

    Ok(())
}

unsafe extern "C" fn ax88179_bind(dev: *mut usbnet, intf: *mut usb_interface) -> c_int {
    try_ax88179_bind(dev, intf).into_kernel_errno()
}

unsafe extern "C" fn ax88179_unbind(dev: *mut usbnet, intf: *mut usb_interface) {
    println!("ax88179_unbind");
    // 	u16 *tmp16;
    // 	u8 *tmp8;
    // 	struct ax88179_data *ax179_data = (struct ax88179_data *) dev->data;

    // 	tmp16 = kmalloc(3, GFP_KERNEL);
    // 	if (!tmp16)
    // 		return;
    // 	tmp8 = (u8*)(&tmp16[2]);

    // 	if (ax179_data) {
    // 		/* Configure RX control register => stop operation */
    // 		*tmp16 = AX_RX_CTL_STOP;
    // 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_CTL, 2, 2, tmp16);

    // 		*tmp8 = 0x0;
    // 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_CLK_SELECT,
    // 				  1, 1, tmp8);

    // 		/* Power down ethernet PHY */
    // 		*tmp16 = 0;
    // 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL,
    // 				  2, 2, tmp16);
    // 		msleep(200);
    // 	}

    // 	kfree(tmp16);
}

// static void
// ax88179_rx_checksum(struct sk_buff *skb, u32 *pkt_hdr)
// {
// 	skb->ip_summed = CHECKSUM_NONE;

// 	/* checksum error bit is set */
// 	if ((*pkt_hdr & AX_RXHDR_L3CSUM_ERR) ||
// 	    (*pkt_hdr & AX_RXHDR_L4CSUM_ERR))
// 		return;

// 	/* It must be a TCP or UDP packet with a valid checksum */
// 	if (((*pkt_hdr & AX_RXHDR_L4_TYPE_MASK) == AX_RXHDR_L4_TYPE_TCP) ||
// 	    ((*pkt_hdr & AX_RXHDR_L4_TYPE_MASK) == AX_RXHDR_L4_TYPE_UDP))
// 		skb->ip_summed = CHECKSUM_UNNECESSARY;
// }

unsafe extern "C" fn ax88179_rx_fixup(dev: *mut usbnet, skb: *mut sk_buff) -> c_int {
    // 	struct sk_buff *ax_skb = NULL;
    // 	int pkt_cnt = 0;
    // 	u32 rx_hdr = 0;
    // 	u16 hdr_off = 0;
    // 	u32 *pkt_hdr = NULL;

    // 	if (skb->len == 0) {
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 		netdev_err(dev->net, "RX SKB length zero");
    // #else
    // 		deverr(dev, "RX SKB length zero");
    // #endif
    // 		dev->net->stats.rx_errors++;
    //     return 0;
    // }

    // 	skb_trim(skb, skb->len - 4);
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 22)
    // 	memcpy(&rx_hdr, skb_tail_pointer(skb), sizeof(rx_hdr));
    // #else
    // 	memcpy(&rx_hdr, skb->tail, sizeof(rx_hdr));
    // #endif
    // 	le32_to_cpus(&rx_hdr);

    // 	pkt_cnt = (u16)rx_hdr;
    // 	hdr_off = (u16)(rx_hdr >> 16);
    // 	pkt_hdr = (u32 *)(skb->data + hdr_off);

    // 	while (pkt_cnt--) {
    // 		u16 pkt_len;

    // 		le32_to_cpus(pkt_hdr);
    // 		pkt_len = (*pkt_hdr >> 16) & 0x1fff;

    // 		/* Check CRC or runt packet */
    // 		if ((*pkt_hdr & AX_RXHDR_CRC_ERR) ||
    // 		    (*pkt_hdr & AX_RXHDR_DROP_ERR)) {
    // 			skb_pull(skb, (pkt_len + 7) & 0xFFF8);
    // 			pkt_hdr++;
    // 			continue;
    // 		}

    // 		if (pkt_cnt == 0) {
    // 			skb->len = pkt_len;

    // 			/* Skip IP alignment psudo header */
    // 			if (NET_IP_ALIGN == 0)
    // 				skb_pull(skb, 2);

    // #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 22)
    // 			skb->tail = skb->data + skb->len;
    // #else
    // 			skb_set_tail_pointer(skb, skb->len);
    // #endif
    // 			skb->truesize = skb->len + sizeof(struct sk_buff);
    // 			ax88179_rx_checksum(skb, pkt_hdr);

    // 			return 1;
    // 		}

    // #ifndef RX_SKB_COPY
    // 		ax_skb = skb_clone(skb, GFP_ATOMIC);
    // #else
    // 		ax_skb = alloc_skb(pkt_len + NET_IP_ALIGN, GFP_ATOMIC);
    // 		skb_reserve(ax_skb, NET_IP_ALIGN);
    // #endif

    // 		if (ax_skb) {
    // #ifndef RX_SKB_COPY
    // 			ax_skb->len = pkt_len;

    // 			/* Skip IP alignment psudo header */
    // 			if (NET_IP_ALIGN == 0)
    // 				skb_pull(ax_skb, 2);

    // #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 22)
    // 			ax_skb->tail = ax_skb->data + ax_skb->len;
    // #else
    // 			skb_set_tail_pointer(ax_skb, ax_skb->len);
    // #endif

    // #else
    // 			skb_put(ax_skb, pkt_len);
    // 			memcpy(ax_skb->data, skb->data, pkt_len);

    // 			if (NET_IP_ALIGN == 0)
    // 				skb_pull(ax_skb, 2);
    // #endif
    // 			ax_skb->truesize = ax_skb->len + sizeof(struct sk_buff);
    // 			ax88179_rx_checksum(ax_skb, pkt_hdr);
    // 			usbnet_skb_return(dev, ax_skb);
    // 		} else {
    // 			return 0;
    // 		}

    // 		skb_pull(skb, (pkt_len + 7) & 0xFFF8);
    // 		pkt_hdr++;
    // 	}
    return 1;
}

unsafe extern "C" fn ax88179_tx_fixup(
    dev: *mut usbnet,
    skb: *mut sk_buff,
    flags: gfp_t,
) -> *mut sk_buff {
    println!("ax88179_tx_fixup");
    // 	u32 tx_hdr1 = 0, tx_hdr2 = 0;
    // 	int frame_size = dev->maxpacket;
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 24)
    // 	int mss = skb_shinfo(skb)->gso_size;
    // #else
    // 	int mss = 0;
    // #endif
    // 	int headroom = 0;
    // 	int tailroom = 0;

    // 	tx_hdr1 = skb->len;
    // 	tx_hdr2 = mss;
    // 	if (((skb->len + 8) % frame_size) == 0)
    // 		tx_hdr2 |= 0x80008000;	/* Enable padding */
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(3, 12, 0)
    // 	if (!dev->can_dma_sg && (dev->net->features & NETIF_F_SG) &&
    // 	    skb_linearize(skb))
    // 		return NULL;
    // #elif LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 24)
    // 	if ((dev->net->features & NETIF_F_SG) && skb_linearize(skb))
    // 		return NULL;
    // #endif

    // 	headroom = skb_headroom(skb);
    // 	tailroom = skb_tailroom(skb);

    // 	if ((headroom + tailroom) >= 8) {
    // 		if (headroom < 8) {
    // 			skb->data = memmove(skb->head + 8, skb->data, skb->len);
    // #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 22)
    // 			skb->tail = skb->data + skb->len;
    // #else
    // 			skb_set_tail_pointer(skb, skb->len);
    // #endif
    // 		}
    // 	} else {
    // 		struct sk_buff *skb2 = NULL;
    // 		skb2 = skb_copy_expand(skb, 8, 0, flags);
    // 		dev_kfree_skb_any(skb);
    // 		skb = skb2;
    // 		if (!skb)
    // 			return NULL;
    // 	}

    // 	skb_push(skb, 4);
    // 	cpu_to_le32s(&tx_hdr2);
    // #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 22)
    // 	memcpy(skb->data, &tx_hdr2, 4);
    // #else
    // 	skb_copy_to_linear_data(skb, &tx_hdr2, 4);
    // #endif

    // 	skb_push(skb, 4);
    // 	cpu_to_le32s(&tx_hdr1);
    // #if LINUX_VERSION_CODE < KERNEL_VERSION(2, 6, 22)
    // 	memcpy(skb->data, &tx_hdr1, 4);
    // #else
    // 	skb_copy_to_linear_data(skb, &tx_hdr1, 4);
    // #endif

    return skb;
}

unsafe extern "C" fn ax88179_link_reset(dev: *mut usbnet) -> c_int {
    println!("ax88179_link_reset");
    // 	struct ax88179_data *data = (struct ax88179_data *)&dev->data;
    // 	u8 *tmp, *link_sts, *tmp_16;
    // 	u16 *mode, *tmp16, delay = 10 * HZ;
    // 	u32 *tmp32;
    // 	unsigned long jtimeout = 0;

    // 	tmp_16 = kmalloc(16, GFP_KERNEL);
    // 	if (!tmp_16)
    // 		return -ENOMEM;
    // 	tmp = (u8*)tmp_16;
    // 	link_sts = (u8*)(&tmp_16[5]);
    // 	mode = (u16*)(&tmp_16[6]);
    // 	tmp16 = (u16*)(&tmp_16[8]);
    // 	tmp32 = (u32*)(&tmp_16[10]);

    // 	*mode = AX_MEDIUM_TXFLOW_CTRLEN | AX_MEDIUM_RXFLOW_CTRLEN;

    // 	ax88179_read_cmd(dev, AX_ACCESS_MAC, PHYSICAL_LINK_STATUS,
    // 			 1, 1, link_sts, 0);

    // 	jtimeout = jiffies + delay;
    // 	while(time_before(jiffies, jtimeout)) {

    // 		ax88179_read_cmd(dev, AX_ACCESS_PHY, AX88179_PHY_ID, GMII_PHY_PHYSR, 2, tmp16, 1);

    // 		if (*tmp16 & GMII_PHY_PHYSR_LINK) {
    // 			break;
    // 		}
    // 	}

    // 	if (!(*tmp16 & GMII_PHY_PHYSR_LINK))
    // 		return 0;
    // 	else if (GMII_PHY_PHYSR_GIGA == (*tmp16 & GMII_PHY_PHYSR_SMASK)) {
    // 		*mode |= AX_MEDIUM_GIGAMODE;
    // 		if (dev->net->mtu > 1500)
    // 			*mode |= AX_MEDIUM_JUMBO_EN;

    // 		if (*link_sts & AX_USB_SS)
    // 			memcpy(tmp, &AX88179_BULKIN_SIZE[0], 5);
    // 		else if (*link_sts & AX_USB_HS)
    // 			memcpy(tmp, &AX88179_BULKIN_SIZE[1], 5);
    // 		else
    // 			memcpy(tmp, &AX88179_BULKIN_SIZE[3], 5);
    // 	} else if (GMII_PHY_PHYSR_100 == (*tmp16 & GMII_PHY_PHYSR_SMASK)) {
    // 		*mode |= AX_MEDIUM_PS;	/* Bit 9 : PS */
    // 		if (*link_sts & (AX_USB_SS | AX_USB_HS))
    // 			memcpy(tmp, &AX88179_BULKIN_SIZE[2], 5);
    // 		else
    // 			memcpy(tmp, &AX88179_BULKIN_SIZE[3], 5);
    // 	} else
    // 		memcpy(tmp, &AX88179_BULKIN_SIZE[3], 5);

    // 	if (bsize != -1) {
    // 		if (bsize > 24)
    // 			bsize = 24;

    // 		else if (bsize == 0) {
    // 			tmp[1] = 0;
    // 			tmp[2] = 0;
    // 		}

    // 		tmp[3] = (u8)bsize;
    // 	}

    // 	if (ifg != -1) {
    // 		if (ifg > 255)
    // 			ifg = 255;
    // 		tmp[4] = (u8)ifg;
    // 	}

    // 	/* RX bulk configuration */
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_BULKIN_QCTRL, 5, 5, tmp);

    // 	if (*tmp16 & GMII_PHY_PHYSR_FULL)
    // 		*mode |= AX_MEDIUM_FULL_DUPLEX;	/* Bit 1 : FD */
    // 	dev->rx_urb_size = (1024 * (tmp[3] + 2));

    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 		netdev_info(dev->net, "Write medium type: 0x%04x\n", *mode);
    // #else
    // 		devinfo(dev, "Write medium type: 0x%04x\n", *mode);
    // #endif

    // 	ax88179_read_cmd(dev, 0x81, 0x8c, 0, 4, tmp32, 1);
    // 	delay = HZ / 2;
    // 	if (*tmp32 & 0x40000000) {

    // 		u16 *tmp1 = (u16*)(&tmp_16[14]);
    // 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_CTL, 2, 2, tmp1);

    // 		/* Configure default medium type => giga */
    // 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
    // 				  2, 2, mode);

    // 		jtimeout = jiffies + delay;

    // 		while (time_before(jiffies, jtimeout)) {

    // 			ax88179_read_cmd(dev, 0x81, 0x8c, 0, 4, tmp32, 1);

    // 			if (!(*tmp32 & 0x40000000))
    // 				break;

    // 			*tmp32 = 0x80000000;
    // 			ax88179_write_cmd(dev, 0x81, 0x8c, 0, 4, tmp32);
    // 		}

    // 		ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_CTL,
    // 				  2, 2, &data->rxctl);
    // 	}

    // 	*mode |= AX_MEDIUM_RECEIVE_EN;

    // 	/* Configure default medium type => giga */
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
    // 			  2, 2, mode);
    // 	mii_check_media(&dev->mii, 1, 1);
    // #if LINUX_VERSION_CODE < KERNEL_VERSION(4, 0, 0)
    // 	if (dev->mii.force_media)
    // 		netif_carrier_on(dev->net);
    // #endif
    // 	kfree(tmp_16);

    return 0;
}

unsafe extern "C" fn ax88179_reset(dev: *mut usbnet) -> c_int {
    println!("ax88179_reset");
    // 	void *buf = NULL;
    // 	u16 *tmp16 = NULL;
    // 	u8 *tmp = NULL;
    // 	struct ax88179_data *ax179_data = (struct ax88179_data *) dev->data;
    // 	buf = kmalloc(6, GFP_KERNEL);

    // 	if (!buf) {
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 		netdev_err(dev->net, "Cannot allocate memory for buffer");
    // #else
    // 		deverr(dev, "Cannot allocate memory for buffer");
    // #endif
    // 		return -ENOMEM;
    // 	}

    // 	tmp16 = (u16 *)buf;
    // 	tmp = (u8 *)buf;

    // 	/* Power up ethernet PHY */
    // 	*tmp16 = 0;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL, 2, 2, tmp16);
    // 	*tmp16 = AX_PHYPWR_RSTCTL_IPRL;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PHYPWR_RSTCTL, 2, 2, tmp16);
    // 	msleep(200);

    // 	*tmp = AX_CLK_SELECT_ACS | AX_CLK_SELECT_BCS;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_CLK_SELECT, 1, 1, tmp);
    // 	msleep(100);

    // 	/* Ethernet PHY Auto Detach*/
    // 	ax88179_AutoDetach(dev, 0);

    // 	/* Set the MAC address */
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_NODE_ID, ETH_ALEN,
    // 			  ETH_ALEN, dev->net->dev_addr);

    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 	netdev_dbg(dev->net, "MAC [%02x-%02x-%02x-%02x-%02x-%02x]\n",
    // 	dev->net->dev_addr[0], dev->net->dev_addr[1],
    // 	dev->net->dev_addr[2], dev->net->dev_addr[3],
    // 	dev->net->dev_addr[4], dev->net->dev_addr[5]);
    // #else
    // 	devdbg(dev, "MAC [%02x-%02x-%02x-%02x-%02x-%02x]\n",
    // 	dev->net->dev_addr[0], dev->net->dev_addr[1],
    // 	dev->net->dev_addr[2], dev->net->dev_addr[3],
    // 	dev->net->dev_addr[4], dev->net->dev_addr[5]);
    // #endif

    // 	/* RX bulk configuration */
    // 	memcpy(tmp, &AX88179_BULKIN_SIZE[0], 5);
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_BULKIN_QCTRL, 5, 5, tmp);

    // 	dev->rx_urb_size = 1024 * 20;

    // 	tmp[0] = 0x34;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PAUSE_WATERLVL_LOW, 1, 1, tmp);

    // 	tmp[0] = 0x52;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_PAUSE_WATERLVL_HIGH,
    // 			  1, 1, tmp);

    // 	dev->net->features |= NETIF_F_IP_CSUM;
    // #if LINUX_VERSION_CODE > KERNEL_VERSION(2, 6, 22)
    // 	dev->net->features |= NETIF_F_IPV6_CSUM;
    // #endif
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(3, 12, 0)
    // 	if (usb_device_no_sg_constraint(dev->udev))
    // 		dev->can_dma_sg = 1;
    // 	dev->net->features |= NETIF_F_SG | NETIF_F_TSO;
    // #endif

    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 39)
    // 	dev->net->hw_features |= NETIF_F_IP_CSUM;
    // 	dev->net->hw_features |= NETIF_F_IPV6_CSUM;
    // 	dev->net->hw_features |= NETIF_F_SG | NETIF_F_TSO;
    // #endif

    // 	/* Enable checksum offload */
    // 	*tmp = AX_RXCOE_IP | AX_RXCOE_TCP | AX_RXCOE_UDP |
    // 	       AX_RXCOE_TCPV6 | AX_RXCOE_UDPV6;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RXCOE_CTL, 1, 1, tmp);

    // 	*tmp = AX_TXCOE_IP | AX_TXCOE_TCP | AX_TXCOE_UDP |
    // 	       AX_TXCOE_TCPV6 | AX_TXCOE_UDPV6;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_TXCOE_CTL, 1, 1, tmp);

    // 	ax179_data->checksum |= AX_RX_CHECKSUM | AX_TX_CHECKSUM;

    // 	/* Configure RX control register => start operation */
    // 	*tmp16 = AX_RX_CTL_DROPCRCERR | AX_RX_CTL_START | AX_RX_CTL_AP |
    // 		 AX_RX_CTL_AMALL | AX_RX_CTL_AB;
    // 	if (NET_IP_ALIGN == 0)
    // 		*tmp16 |= AX_RX_CTL_IPE;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_RX_CTL, 2, 2, tmp16);

    // 	*tmp = AX_MONITOR_MODE_PMETYPE | AX_MONITOR_MODE_PMEPOL |
    // 						AX_MONITOR_MODE_RWMP;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MONITOR_MODE, 1, 1, tmp);

    // 	ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_MONITOR_MODE, 1, 1, tmp, 0);
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 	netdev_dbg(dev->net, "Monitor mode = 0x%02x\n", *tmp);
    // #else
    // 	devdbg(dev, "Monitor mode = 0x%02x\n", *tmp);
    // #endif

    // 	/* Configure default medium type => giga */
    // 	*tmp16 = AX_MEDIUM_TXFLOW_CTRLEN | AX_MEDIUM_RXFLOW_CTRLEN |
    // 		 AX_MEDIUM_FULL_DUPLEX | AX_MEDIUM_GIGAMODE;

    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
    // 			  2, 2, tmp16);

    // 	ax88179_led_setting(dev);

    // 	ax88179_EEE_setting(dev);

    // 	ax88179_Gether_setting(dev);

    // 	/* Restart autoneg */
    // 	mii_nway_restart(&dev->mii);

    // 	netif_carrier_off(dev->net);

    // 	kfree(buf);
    // #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 34)
    // 	netdev_dbg(dev->net, "mtu %d\n", dev->net->mtu);
    // #else
    // 	devdbg(dev, "mtu %d\n", dev->net->mtu);
    // #endif

    return 0;
}

unsafe extern "C" fn ax88179_stop(dev: *mut usbnet) -> c_int {
    // 	u16 *tmp16;
    // 	tmp16 = kmalloc(2, GFP_KERNEL);
    // 	if (!tmp16)
    // 		return -ENOMEM;

    // 	ax88179_read_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
    // 			 2, 2, tmp16, 1);
    // 	*tmp16 &= ~AX_MEDIUM_RECEIVE_EN;
    // 	ax88179_write_cmd(dev, AX_ACCESS_MAC, AX_MEDIUM_STATUS_MODE,
    // 			  2, 2, tmp16);

    // 	kfree(tmp16);
    return 0;
}

trait KernelResultExt {
    fn from_kernel_errno(errno: c_int) -> Self;
    fn into_kernel_errno(self) -> c_int;
}

impl KernelResultExt for KernelResult<()> {
    fn from_kernel_errno(errno: c_int) -> Self {
        match errno {
            0 => Ok(()),
            _ => Err(Error::from_kernel_errno(errno))
        }
    }

    fn into_kernel_errno(self) -> c_int {
        match self {
            Ok(()) => 0,
            Err(e) => e.into_kernel_errno(),
        }
    }
}

#[allow(non_upper_case_globals)]
static mut ax88179_info: MaybeUninit<driver_info> = MaybeUninit::uninit();

// static const struct driver_info ax88178a_info = {
// 	.description = "",
// //	.description = "ASIX AX88178A USB 2.0 Gigabit Ethernet",
// 	.bind	= ax88179_bind,
// 	.unbind	= ax88179_unbind,
// 	.status	= ax88179_status,
// 	.link_reset = ax88179_link_reset,
// 	.reset	= ax88179_reset,
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 32)
// 	.stop	= ax88179_stop,
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX | FLAG_AVOID_UNLINK_URBS,
// #else
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX,
// #endif
// 	.rx_fixup = ax88179_rx_fixup,
// 	.tx_fixup = ax88179_tx_fixup,
// };

// static const struct driver_info sitecom_info = {
// 	.description = "Sitecom USB 3.0 to Gigabit Adapter",
// 	.bind	= ax88179_bind,
// 	.unbind	= ax88179_unbind,
// 	.status	= ax88179_status,
// 	.link_reset = ax88179_link_reset,
// 	.reset	= ax88179_reset,
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 32)
// 	.stop	= ax88179_stop,
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX | FLAG_AVOID_UNLINK_URBS,
// #else
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX,
// #endif
// 	.rx_fixup = ax88179_rx_fixup,
// 	.tx_fixup = ax88179_tx_fixup,
// };

// static const struct driver_info lenovo_info = {
// 	.description = "ThinkPad OneLinkDock USB GigaLAN",
// 	.bind	= ax88179_bind,
// 	.unbind	= ax88179_unbind,
// 	.status	= ax88179_status,
// 	.link_reset = ax88179_link_reset,
// 	.reset	= ax88179_reset,
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 32)
// 	.stop	= ax88179_stop,
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX | FLAG_AVOID_UNLINK_URBS,
// #else
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX,
// #endif
// 	.rx_fixup = ax88179_rx_fixup,
// 	.tx_fixup = ax88179_tx_fixup,
// };

// static const struct driver_info toshiba_info = {
// 	.description = "Toshiba USB 3.0 to Gigabit LAN Adapter",
// 	.bind	= ax88179_bind,
// 	.unbind	= ax88179_unbind,
// 	.status	= ax88179_status,
// 	.link_reset = ax88179_link_reset,
// 	.reset	= ax88179_reset,
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 32)
// 	.stop	= ax88179_stop,
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX | FLAG_AVOID_UNLINK_URBS,
// #else
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX,
// #endif
// 	.rx_fixup = ax88179_rx_fixup,
// 	.tx_fixup = ax88179_tx_fixup,
// };

// static const struct driver_info samsung_info = {
// 	.description = "Samsung USB Ethernet Adapter",
// 	.bind	= ax88179_bind,
// 	.unbind = ax88179_unbind,
// 	.status = ax88179_status,
// 	.link_reset = ax88179_link_reset,
// 	.reset	= ax88179_reset,
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 32)
// 	.stop	= ax88179_stop,
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX | FLAG_AVOID_UNLINK_URBS,
// #else
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX,
// #endif
// 	.rx_fixup = ax88179_rx_fixup,
// 	.tx_fixup = ax88179_tx_fixup,
// };

// static const struct driver_info dlink_info = {
// 	.description = "DUB-1312/1332 USB3.0 to Gigabit Ethernet Adapter",
// 	.bind	= ax88179_bind,
// 	.unbind = ax88179_unbind,
// 	.status = ax88179_status,
// 	.link_reset = ax88179_link_reset,
// 	.reset	= ax88179_reset,
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 32)
// 	.stop	= ax88179_stop,
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX | FLAG_AVOID_UNLINK_URBS,
// #else
// 	.flags	= FLAG_ETHER | FLAG_FRAMING_AX,
// #endif
// 	.rx_fixup = ax88179_rx_fixup,
// 	.tx_fixup = ax88179_tx_fixup,
// };

// static const struct driver_info mct_info = {
// 	.description = "USB 3.0 to Gigabit Ethernet Adapter",
// 	.bind   = ax88179_bind,
// 	.unbind = ax88179_unbind,
// 	.status = ax88179_status,
// 	.link_reset = ax88179_link_reset,
// 	.reset  = ax88179_reset,
// #if LINUX_VERSION_CODE >= KERNEL_VERSION(2, 6, 32)
// 	.stop   = ax88179_stop,
// 	.flags  = FLAG_ETHER | FLAG_FRAMING_AX | FLAG_AVOID_UNLINK_URBS,
// #else
// 	.flags  = FLAG_ETHER | FLAG_FRAMING_AX,
// #endif
// 	.rx_fixup = ax88179_rx_fixup,
// 	.tx_fixup = ax88179_tx_fixup,
// };

#[export_name = "__mod_usb__products_device_table"]
static mut PRODUCTS: MaybeUninit<[usb_device_id; 2]> = MaybeUninit::uninit();

fn get_driver_info() -> usb_driver {
    // TODO: wrap all this static_mut initialisation in a call_once
    unsafe {
        ax88179_info.as_mut_ptr().write(driver_info {
            description: "ASIX AX88179 USB 3.0 Gigabit Ethernet\0".as_ptr() as _,
            bind: Some(ax88179_bind),
            unbind: Some(ax88179_unbind),
            status: Some(ax88179_status),
            link_reset: Some(ax88179_link_reset),
            reset: Some(ax88179_reset),
            stop: Some(ax88179_stop),
            flags: (FLAG_ETHER | FLAG_FRAMING_AX | FLAG_AVOID_UNLINK_URBS) as _,
            rx_fixup: Some(ax88179_rx_fixup),
            tx_fixup: Some(ax88179_tx_fixup),
            ..Default::default()
        });

        PRODUCTS.as_mut_ptr().write([
            // ASIX AX88179 10/100/1000
            usb_device_id {
                match_flags: USB_DEVICE_ID_MATCH_DEVICE as _,
                idVendor: 0x0b95,
                idProduct: 0x1790,
                driver_info: ax88179_info.as_ptr() as _,
                ..Default::default()
            },
            // {
            // 	/* ASIX AX88179 10/100/1000 */
            // 	USB_DEVICE(0x0b95, 0x1790),
            // 	.driver_info = (unsigned long) &ax88179_info,
            // }, {
            // 	/* ASIX AX88178A 10/100/1000 */
            // 	USB_DEVICE(0x0b95, 0x178a),
            // 	.driver_info = (unsigned long) &ax88178a_info,
            // }, {
            // 	/* Sitecom USB 3.0 to Gigabit Adapter */
            // 	USB_DEVICE(0x0df6, 0x0072),
            // 	.driver_info = (unsigned long) &sitecom_info,
            // }, {
            // 	/* ThinkPad OneLinkDock USB GigaLAN */
            // 	USB_DEVICE(0x17ef, 0x304b),
            // 	.driver_info = (unsigned long) &lenovo_info,
            // }, {
            // 	/* Toshiba USB3.0 to Gigabit LAN Adapter */
            // 	USB_DEVICE(0x0930, 0x0a13),
            // 	.driver_info = (unsigned long) &toshiba_info,
            // }, {
            // 	/* Samsung USB Ethernet Adapter */
            // 	USB_DEVICE(0x04e8, 0xa100),
            // 	.driver_info = (unsigned long) &samsung_info,
            // }, {
            // 	/* D-Link DUB-13x2 Ethernet Adapter */
            // 	USB_DEVICE(0x2001, 0x4a00),
            // 	.driver_info = (unsigned long) &dlink_info,
            // }, {
            // 	/* MCT USB 3.0 to Gigabit Ethernet Adapter */
            // 	USB_DEVICE(0x0711, 0x0179),
            // 	.driver_info = (unsigned long) &mct_info,
            // },

            // End sentinel
            core::mem::zeroed(),
        ]);
    }

    usb_driver {
        name: "ax88179_178a\0".as_ptr() as _,
        id_table: unsafe { &*PRODUCTS.as_ptr() }.as_ptr(),
        probe: Some(usbnet_probe),
        suspend: Some(ax88179_suspend),
        resume: Some(ax88179_resume),
        disconnect: Some(usbnet_disconnect),
        ..Default::default()
    }
}

linux_kernel_module::kernel_module!(
    ax88179_178a_module,
    author: DRIVER_AUTHOR,
    description: DRIVER_DESCRIPTION,
    license: DRIVER_LICENSE
);
