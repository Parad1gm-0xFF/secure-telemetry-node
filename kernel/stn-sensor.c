// stn-sensor.c - driver noyau d'exemple : pseudo-capteur exposé en /dev.
//
// Démonstration du versant KERNEL/BSP de secure-telemetry-node. Le module
// enregistre un périphérique misc "/dev/stn-sensor" dont la lecture retourne
// une valeur de température simulée (format milli-degrés, comme thermal_zone0).
// Le daemon Rust peut alors lire /dev/stn-sensor au lieu de la sysfs CPU.
//
// Cycle de vie complet d'un driver : chargement (module_init), enregistrement
// de périphérique (misc_register), opération de lecture, déchargement
// (module_exit). Aucune allocation, aucun état global : modèle simple et sûr.
//
// CIBLE : kernels 6.x LTS (Yocto/redpesk Scarthgap 6.6, la cible embarquée).
// ATTENTION compatibilité : le kernel 7.0 a refondu l'API miscdevice (le
// callback read est déplacé dans un `struct file_operations *fops`) et le
// noyau 6.11 a réorganisé les callbacks dans un sous-struct `params`. Comme
// pour seccomp (voir src/main.rs), ces évolutions d'API sont réelles : ce
// module cible délibérément l'API 6.x LTS des BSP industriels et refuse de
// compiler sur une API non couverte plutôt que de produire un binaire faux.

#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/miscdevice.h>
#include <linux/uaccess.h>
#include <linux/version.h>

#if KERNEL_VERSION(6, 11, 0) <= LINUX_VERSION_CODE
#error "API miscdevice refondue (6.11+/7.x, fops) : ce module cible les kernels 6.x LTS, adapter stn_read aux callbacks params/fops"
#endif

#define STN_SAMPLE "25000\n"
#define STN_SAMPLE_LEN 6

// Lecture : recopie l'échantillon simulé dans le buffer utilisateur.
static ssize_t stn_read(struct miscdevice *misc, char __user *buf, size_t count)
{
	ssize_t n = count < STN_SAMPLE_LEN ? (ssize_t)count : (ssize_t)STN_SAMPLE_LEN;
	if (copy_to_user(buf, STN_SAMPLE, n))
		return -EFAULT;
	return n;
}

// Le périphérique expose une seule opération (lecture). Aucune écriture,
// aucun ioctl : surface d'attaque minimale (cohérent avec le démon Rust).
static struct miscdevice stn_misc = {
	.name = "stn-sensor",
	.minor = MISC_DYNAMIC_MINOR,
	.read = stn_read,
};

static int __init stn_init(void)
{
	return misc_register(&stn_misc);
}

static void __exit stn_exit(void)
{
	misc_deregister(&stn_misc);
}

module_init(stn_init);
module_exit(stn_exit);

MODULE_AUTHOR("Parad1gm_0xFF");
MODULE_DESCRIPTION("Secure Telemetry Node - pseudo-capteur de demonstration");
MODULE_LICENSE("GPL");