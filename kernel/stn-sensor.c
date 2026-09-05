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
// API miscdevice : l'interface est stable depuis longtemps. La structure
// `miscdevice` référence une table `struct file_operations *fops` (depuis
// v4.19 au moins, inchangée sur 6.x et 7.x). Les opérations (read, write...)
// se déclarent dans cette table, PAS directement dans miscdevice.

#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/miscdevice.h>
#include <linux/fs.h>
#include <linux/uaccess.h>

#define STN_SAMPLE "25000\n"
#define STN_SAMPLE_LEN 6

// Lecture : recopie l'échantillon simulé dans le buffer utilisateur.
// Signature de l'API file_operations : (struct file *, char __user *, size_t, loff_t *).
static ssize_t stn_read(struct file *file, char __user *buf, size_t count, loff_t *pos)
{
	ssize_t n = count < STN_SAMPLE_LEN ? (ssize_t)count : (ssize_t)STN_SAMPLE_LEN;
	if (copy_to_user(buf, STN_SAMPLE, n))
		return -EFAULT;
	if (pos)
		*pos += n;
	return n;
}

// Table des opérations du périphérique : une seule opération (lecture).
// Aucune écriture, aucun ioctl : surface d'attaque minimale (cohérent avec le
// démon Rust). Le champ owner est requis par l'API récente (6.x/7.x).
static const struct file_operations stn_fops = {
	.owner = THIS_MODULE,
	.read = stn_read,
};

// Le périphérique misc référence la table d'opérations via fops.
static struct miscdevice stn_misc = {
	.name = "stn-sensor",
	.minor = MISC_DYNAMIC_MINOR,
	.fops = &stn_fops,
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