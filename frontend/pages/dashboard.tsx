"use client"

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { useUser } from '@clerk/nextjs';
import { useRouter } from 'next/navigation';
import {
    BookOpen, Plus, Trash2, Pencil, X, ExternalLink,
    FileText, ChevronRight, AlertCircle, Loader2, UserPlus,
    Mail, Copy, Check, Send
} from 'lucide-react';
import Footer from '../components/Footer';
import AdminNav from '@/components/AdminNav';
import { useApiClient, ApiError } from '@/lib/api';
import { useBackendSync } from '@/hooks/useBackendSync';

interface Author {
    name: string;
    email: string;
}

interface Paper {
    id: string;
    title: string;
    authors: Author[];
    abstract_text: string;
    paper_url: string;
    pdf_url?: string;
}

interface PaperForm {
    title: string;
    authors: Author[];
    abstract_text: string;
    paper_url: string;
    pdf_url?: string;
}

const emptyAuthor: Author = { name: '', email: '' };
const emptyForm: PaperForm = {
    title: '',
    authors: [{ ...emptyAuthor }],
    abstract_text: '',
    paper_url: '',
    pdf_url: '',
};

export default function AdminDashboard() {
    const { user, isLoaded } = useUser();
    const router = useRouter();
    const api = useApiClient();
    useBackendSync();

    const [papers, setPapers] = useState<Paper[]>([]);
    const [loading, setLoading] = useState(true);
    const [modalOpen, setModalOpen] = useState(false);
    const [editingPaper, setEditingPaper] = useState<Paper | null>(null);
    const [form, setForm] = useState<PaperForm>(emptyForm);
    const [submitting, setSubmitting] = useState(false);
    const [deleteId, setDeleteId] = useState<string | null>(null);
    const [error, setError] = useState('');

    const [invitingPaper, setInvitingPaper] = useState<Paper | null>(null);
    const [inviteEmail, setInviteEmail] = useState('');
    const [inviteName, setInviteName] = useState('');
    const [inviteBio, setInviteBio] = useState('');
    const [inviteSubmitting, setInviteSubmitting] = useState(false);
    const [inviteSuccess, setInviteSuccess] = useState(false);
    const [inviteError, setInviteError] = useState('');
    const [linkCopied, setLinkCopied] = useState(false);

    // Guard: only admins
    useEffect(() => {
        if (isLoaded && user?.publicMetadata?.role !== 'admin') {
            router.replace('/');
        }
    }, [isLoaded, user, router]);

    useEffect(() => {
        fetchPapers();
    }, []);

    async function fetchPapers() {
        setLoading(true);
        try {
            const data = await api.get<Paper[]>('/papers');
            setPapers(data);
        } catch {
            setError('Failed to load papers.');
        } finally {
            setLoading(false);
        }
    }

    function openAdd() {
        setEditingPaper(null);
        setForm({ ...emptyForm, authors: [{ ...emptyAuthor }] });
        setModalOpen(true);
    }

    function openEdit(paper: Paper) {
        setEditingPaper(paper);
        setForm({
            title: paper.title,
            authors: paper.authors?.length ? paper.authors.map(a => ({ ...a })) : [{ ...emptyAuthor }],
            abstract_text: paper.abstract_text,
            paper_url: paper.paper_url,
            pdf_url: paper.pdf_url,
        });
        setModalOpen(true);
    }

    function closeModal() {
        setModalOpen(false);
        setEditingPaper(null);
        setForm({ ...emptyForm, authors: [{ ...emptyAuthor }] });
    }

    function addAuthor() {
        setForm(prev => ({ ...prev, authors: [...prev.authors, { ...emptyAuthor }] }));
    }

    function removeAuthor(index: number) {
        setForm(prev => ({ ...prev, authors: prev.authors.filter((_, i) => i !== index) }));
    }

    function updateAuthor(index: number, field: keyof Author, value: string) {
        setForm(prev => {
            const updated = [...prev.authors];
            updated[index] = { ...updated[index], [field]: value };
            return { ...prev, authors: updated };
        });
    }

    async function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        setSubmitting(true);
        setError('');
        try {
            const validAuthors = form.authors.filter(a => a.name.trim());
            console.log('Submitting paper with authors:', validAuthors);
            const payload = {
                title: form.title.trim(),
                abstract_text: form.abstract_text.trim(),
                paper_url: form.paper_url.trim(),
                pdf_url: form.pdf_url?.trim() || undefined,
                authors: validAuthors,
            };

            if (editingPaper) {
                await api.put(`/papers/${editingPaper.id}`, payload);
            } else {
                await api.post('/papers', payload);
            }
            await fetchPapers();
            closeModal();
        } catch (err) {
            setError(err instanceof ApiError ? err.message : 'Failed to save paper.');
        } finally {
            setSubmitting(false);
        }
    }

    async function handleDelete(id: string) {
        setDeleteId(id);
        try {
            await api.delete(`/papers/${id}`);
            setPapers(prev => prev.filter(p => p.id !== id));
        } catch {
            setError('Failed to delete paper.');
        } finally {
            setDeleteId(null);
        }
    }

    function openInvite(paper: Paper) {
        setInvitingPaper(paper);
        setInviteEmail('');
        setInviteName('');
        setInviteBio('');
        setInviteSuccess(false);
        setInviteError('');
        setLinkCopied(false);
    }

    function closeInvite() {
        setInvitingPaper(null);
        setInviteSuccess(false);
        setInviteError('');
    }

    async function handleInviteSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!invitingPaper || !inviteEmail.trim()) return;

        setInviteSubmitting(true);
        setInviteError('');
        try {
            await api.post('/experts', {
                email: inviteEmail.trim(),
                name: inviteName.trim() || inviteEmail.trim(),
                bio: inviteBio.trim(),
                paper_id: invitingPaper.id,
            });
            setInviteSuccess(true);
        } catch (err) {
            setInviteError(err instanceof ApiError ? err.message : 'Failed to invite expert.');
        } finally {
            setInviteSubmitting(false);
        }
    }

    if (!isLoaded) return null;

    return (
        <div
            className="min-h-screen bg-[#f5f0eb] text-[#2c2217] flex flex-col"
            style={{ fontFamily: "'Georgia', 'Times New Roman', serif" }}
        >
            <header className="flex items-center justify-between px-6 py-3 border-b border-[#e0d8cf] bg-[#f5f0eb] shrink-0">
                <Link href="/" className="flex items-center gap-2">
                    <div className="w-7 h-7 rounded-md bg-[#6b1f2a] flex items-center justify-center">
                        <BookOpen className="w-4 h-4 text-white" />
                    </div>
                    <span className="text-[15px] font-semibold tracking-tight">LivePaper</span>
                </Link>
                <AdminNav />
            </header>

            <main className="flex-1 max-w-6xl mx-auto w-full px-6 py-10">
                <div className="flex items-center justify-between mb-8">
                    <div>
                        <div className="flex items-center gap-2 text-xs text-[#8a7060] mb-1" style={{ fontFamily: 'system-ui, sans-serif' }}>
                            <span>Admin</span>
                            <ChevronRight className="w-3 h-3" />
                            <span>Papers</span>
                        </div>
                        <h1 className="text-2xl font-bold text-[#2c2217]">Research Papers</h1>
                    </div>
                    <button
                        onClick={openAdd}
                        className="inline-flex items-center gap-2 px-4 py-2.5 rounded-lg bg-[#6b1f2a] text-white text-sm font-semibold hover:bg-[#4e1520] active:scale-95 transition-all duration-150 shadow-sm"
                        style={{ fontFamily: 'system-ui, sans-serif' }}
                    >
                        <Plus className="w-4 h-4" /> Add Paper
                    </button>
                </div>

                {error && (
                    <div className="flex items-center gap-2 bg-red-50 border border-red-200 text-red-700 rounded-xl px-4 py-3 mb-6 text-sm" style={{ fontFamily: 'system-ui, sans-serif' }}>
                        <AlertCircle className="w-4 h-4 shrink-0" />
                        {error}
                        <button onClick={() => setError('')} className="ml-auto"><X className="w-4 h-4" /></button>
                    </div>
                )}

                <div className="bg-white rounded-2xl border border-[#e0d8cf] overflow-hidden shadow-sm">
                    {loading ? (
                        <div className="flex items-center justify-center py-20 text-[#8a7060]" style={{ fontFamily: 'system-ui, sans-serif' }}>
                            <Loader2 className="w-5 h-5 animate-spin mr-2" /> Loading papers…
                        </div>
                    ) : papers.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-20 text-[#8a7060]" style={{ fontFamily: 'system-ui, sans-serif' }}>
                            <FileText className="w-10 h-10 mb-3 text-[#d4c8bc]" />
                            <p className="text-sm">No papers yet. Add your first paper.</p>
                        </div>
                    ) : (
                        <table className="w-full text-sm" style={{ fontFamily: 'system-ui, sans-serif' }}>
                            <thead>
                                <tr className="border-b border-[#e0d8cf] bg-[#faf7f4]">
                                    <th className="text-left px-5 py-3.5 text-xs font-semibold text-[#8a7060] uppercase tracking-wider">Title</th>
                                    <th className="text-left px-5 py-3.5 text-xs font-semibold text-[#8a7060] uppercase tracking-wider hidden lg:table-cell">Abstract</th>
                                    <th className="text-left px-5 py-3.5 text-xs font-semibold text-[#8a7060] uppercase tracking-wider">Links</th>
                                    <th className="px-5 py-3.5 text-xs font-semibold text-[#8a7060] uppercase tracking-wider text-right">Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                {papers.map((paper, i) => (
                                    <tr key={paper.id} className={`border-b border-[#f0e8e0] hover:bg-[#faf7f4] transition-colors ${i === papers.length - 1 ? 'border-0' : ''}`}>
                                        <td className="px-5 py-4 max-w-[200px]">
                                            <p className="font-semibold text-[#2c2217] line-clamp-2 leading-snug" style={{ fontFamily: "'Georgia', serif" }}>
                                                {paper.title}
                                            </p>
                                        </td>
                                        <td className="px-5 py-4 text-[#8a7060] hidden lg:table-cell max-w-[260px]">
                                            <p className="line-clamp-2 text-xs leading-relaxed">{paper.abstract_text}</p>
                                        </td>
                                        <td className="px-5 py-4">
                                            <div className="flex flex-col gap-1">
                                                {paper.paper_url && (
                                                    <a href={paper.paper_url} target="_blank" rel="noopener noreferrer"
                                                        className="inline-flex items-center gap-1 text-xs text-[#6b1f2a] hover:underline">
                                                        <ExternalLink className="w-3 h-3" /> URL
                                                    </a>
                                                )}
                                                {paper.pdf_url && (
                                                    <a href={paper.pdf_url} target="_blank" rel="noopener noreferrer"
                                                        className="inline-flex items-center gap-1 text-xs text-[#6b1f2a] hover:underline">
                                                        <FileText className="w-3 h-3" /> PDF
                                                    </a>
                                                )}
                                            </div>
                                        </td>
                                        <td className="px-5 py-4">
                                            <div className="flex items-center justify-end gap-2">
                                                <button
                                                    onClick={() => openInvite(paper)}
                                                    className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-[#d4c8bc] text-[#6b1f2a] text-xs font-medium hover:bg-[#f5e6e8] hover:border-[#6b1f2a] transition-all duration-150"
                                                >
                                                    <Mail className="w-3 h-3" /> Invite
                                                </button>
                                                <button
                                                    onClick={() => openEdit(paper)}
                                                    className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-[#d4c8bc] text-[#5a4535] text-xs font-medium hover:bg-[#ede6dc] hover:border-[#6b1f2a] transition-all duration-150"
                                                >
                                                    <Pencil className="w-3 h-3" /> Edit
                                                </button>
                                                <button
                                                    onClick={() => handleDelete(paper.id)}
                                                    disabled={deleteId === paper.id}
                                                    className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-red-200 text-red-600 text-xs font-medium hover:bg-red-50 transition-all duration-150 disabled:opacity-40"
                                                >
                                                    {deleteId === paper.id ? <Loader2 className="w-3 h-3 animate-spin" /> : <Trash2 className="w-3 h-3" />}
                                                    Delete
                                                </button>
                                            </div>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    )}
                </div>

                <p className="text-center text-xs text-[#b0a090] mt-6" style={{ fontFamily: 'system-ui, sans-serif' }}>
                    {papers.length} paper{papers.length !== 1 ? 's' : ''} total
                </p>
            </main>

            <div className="border-t border-[#e0d8cf]">
                <Footer />
            </div>

            {/* Add / Edit Modal */}
            {modalOpen && (
                <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
                    <div className="absolute inset-0 bg-[#2c2217]/40 backdrop-blur-sm" onClick={closeModal} />
                    <div className="relative bg-[#f5f0eb] rounded-2xl border border-[#e0d8cf] shadow-xl w-full max-w-xl max-h-[90vh] overflow-y-auto" style={{ fontFamily: 'system-ui, sans-serif' }}>
                        <div className="flex items-center justify-between px-6 py-4 border-b border-[#e0d8cf]">
                            <h2 className="text-[15px] font-semibold text-[#2c2217]" style={{ fontFamily: "'Georgia', serif" }}>
                                {editingPaper ? 'Update Paper' : 'Add New Paper'}
                            </h2>
                            <button onClick={closeModal} className="text-[#8a7060] hover:text-[#2c2217] transition-colors">
                                <X className="w-5 h-5" />
                            </button>
                        </div>

                        <form onSubmit={handleSubmit} className="px-6 py-5 space-y-5">
                            <div>
                                <label className="block text-xs font-semibold text-[#5a4535] uppercase tracking-wider mb-1.5">
                                    Title <span className="text-[#6b1f2a]">*</span>
                                </label>
                                <input
                                    type="text"
                                    value={form.title}
                                    onChange={e => setForm(prev => ({ ...prev, title: e.target.value }))}
                                    placeholder="Full paper title"
                                    required
                                    className="w-full px-4 py-2.5 rounded-lg border border-[#d4c8bc] bg-white text-[#2c2217] placeholder-[#b0a090] text-sm focus:outline-none focus:border-[#6b1f2a] focus:ring-1 focus:ring-[#6b1f2a] transition-all"
                                />
                            </div>

                            <div>
                                <div className="flex items-center justify-between mb-2">
                                    <label className="text-xs font-semibold text-[#5a4535] uppercase tracking-wider">Authors</label>
                                    <button type="button" onClick={addAuthor} className="inline-flex items-center gap-1 text-xs text-[#6b1f2a] font-medium hover:text-[#4e1520] transition-colors">
                                        <UserPlus className="w-3.5 h-3.5" /> Add Author
                                    </button>
                                </div>
                                <div className="space-y-2">
                                    {form.authors.map((author, index) => (
                                        <div key={index} className="flex gap-2 items-start bg-white border border-[#e0d8cf] rounded-xl p-3">
                                            <div className="w-5 h-5 rounded-full bg-[#6b1f2a] text-white text-[10px] font-bold flex items-center justify-center shrink-0 mt-2">
                                                {index + 1}
                                            </div>
                                            <div className="flex-1 space-y-2">
                                                <input
                                                    type="text"
                                                    value={author.name}
                                                    onChange={e => updateAuthor(index, 'name', e.target.value)}
                                                    placeholder="Author name"
                                                    className="w-full px-3 py-2 rounded-lg border border-[#d4c8bc] bg-[#faf7f4] text-[#2c2217] placeholder-[#b0a090] text-sm focus:outline-none focus:border-[#6b1f2a] transition-all"
                                                />
                                                <input
                                                    type="email"
                                                    value={author.email}
                                                    onChange={e => updateAuthor(index, 'email', e.target.value)}
                                                    placeholder="author@email.com (optional)"
                                                    className="w-full px-3 py-2 rounded-lg border border-[#d4c8bc] bg-[#faf7f4] text-[#2c2217] placeholder-[#b0a090] text-sm focus:outline-none focus:border-[#6b1f2a] transition-all"
                                                />
                                            </div>
                                            {form.authors.length > 1 && (
                                                <button type="button" onClick={() => removeAuthor(index)} className="text-[#b0a090] hover:text-red-500 transition-colors mt-2 shrink-0">
                                                    <X className="w-4 h-4" />
                                                </button>
                                            )}
                                        </div>
                                    ))}
                                </div>
                                <button
                                    type="button"
                                    onClick={addAuthor}
                                    className="mt-2 w-full py-2 rounded-lg border border-dashed border-[#d4c8bc] text-xs text-[#8a7060] hover:border-[#6b1f2a] hover:text-[#6b1f2a] transition-all duration-150 flex items-center justify-center gap-1.5"
                                >
                                    <Plus className="w-3.5 h-3.5" /> Add another author
                                </button>
                            </div>

                            <div>
                                <label className="block text-xs font-semibold text-[#5a4535] uppercase tracking-wider mb-1.5">
                                    Abstract <span className="text-[#6b1f2a]">*</span>
                                </label>
                                <textarea
                                    value={form.abstract_text}
                                    onChange={e => setForm(prev => ({ ...prev, abstract_text: e.target.value }))}
                                    placeholder="Paper abstract…"
                                    required
                                    rows={4}
                                    className="w-full px-4 py-2.5 rounded-lg border border-[#d4c8bc] bg-white text-[#2c2217] placeholder-[#b0a090] text-sm focus:outline-none focus:border-[#6b1f2a] focus:ring-1 focus:ring-[#6b1f2a] transition-all resize-none"
                                />
                            </div>

                            {[
                                { label: 'Paper URL', key: 'paper_url', placeholder: 'https://…' },
                                { label: 'PDF URL', key: 'pdf_url', placeholder: 'https://…/paper.pdf' },
                            ].map(({ label, key, placeholder }) => (
                                <div key={key}>
                                    <label className="block text-xs font-semibold text-[#5a4535] uppercase tracking-wider mb-1.5">{label}</label>
                                    <input
                                        type="text"
                                        value={form[key as keyof PaperForm] as string}
                                        onChange={e => setForm(prev => ({ ...prev, [key]: e.target.value }))}
                                        placeholder={placeholder}
                                        className="w-full px-4 py-2.5 rounded-lg border border-[#d4c8bc] bg-white text-[#2c2217] placeholder-[#b0a090] text-sm focus:outline-none focus:border-[#6b1f2a] focus:ring-1 focus:ring-[#6b1f2a] transition-all"
                                    />
                                </div>
                            ))}

                            <div className="flex gap-3 pt-2">
                                <button
                                    type="button"
                                    onClick={closeModal}
                                    className="flex-1 px-4 py-2.5 rounded-lg border border-[#d4c8bc] text-[#5a4535] text-sm font-medium hover:bg-[#ede6dc] transition-all duration-150"
                                >
                                    Cancel
                                </button>
                                <button
                                    type="submit"
                                    disabled={submitting}
                                    className="flex-1 px-4 py-2.5 rounded-lg bg-[#6b1f2a] text-white text-sm font-semibold hover:bg-[#4e1520] active:scale-95 transition-all duration-150 disabled:opacity-50 inline-flex items-center justify-center gap-2"
                                >
                                    {submitting && <Loader2 className="w-4 h-4 animate-spin" />}
                                    {editingPaper ? 'Update Paper' : 'Add Paper'}
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            )}

            {/* Invite Expert Modal */}
            {invitingPaper && (
                <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
                    <div className="absolute inset-0 bg-[#2c2217]/40 backdrop-blur-sm" onClick={closeInvite} />
                    <div className="relative bg-[#f5f0eb] rounded-2xl border border-[#e0d8cf] shadow-xl w-full max-w-lg max-h-[90vh] overflow-y-auto" style={{ fontFamily: 'system-ui, sans-serif' }}>
                        <div className="flex items-center justify-between px-6 py-4 border-b border-[#e0d8cf]">
                            <div>
                                <h2 className="text-[15px] font-semibold text-[#2c2217]" style={{ fontFamily: "'Georgia', serif" }}>
                                    Invite an Expert
                                </h2>
                                <p className="text-xs text-[#8a7060] mt-0.5 line-clamp-1">{invitingPaper.title}</p>
                            </div>
                            <button onClick={closeInvite} className="text-[#8a7060] hover:text-[#2c2217] transition-colors shrink-0 ml-2">
                                <X className="w-5 h-5" />
                            </button>
                        </div>

                        {inviteSuccess ? (
                            <div className="px-6 py-8 text-center space-y-3">
                                <div className="w-12 h-12 rounded-full bg-green-100 flex items-center justify-center mx-auto">
                                    <Check className="w-6 h-6 text-green-600" />
                                </div>
                                <p className="font-semibold text-[#2c2217]">Expert invited!</p>
                                <p className="text-sm text-[#8a7060]">
                                    An invitation email has been sent to <span className="font-medium text-[#2c2217]">{inviteEmail}</span>.
                                    They&apos;ll receive the paper details and can submit their response directly.
                                </p>
                                <button
                                    onClick={closeInvite}
                                    className="mt-2 w-full py-2.5 rounded-lg border border-[#d4c8bc] text-[#5a4535] text-sm font-medium hover:bg-[#ede6dc] transition-all duration-150"
                                >
                                    Done
                                </button>
                            </div>
                        ) : (
                            <form onSubmit={handleInviteSubmit} className="px-6 py-5 space-y-4">
                                <div>
                                    <label className="block text-xs font-semibold text-[#5a4535] uppercase tracking-wider mb-1.5">
                                        Expert Email <span className="text-[#6b1f2a]">*</span>
                                    </label>
                                    <input
                                        type="email"
                                        value={inviteEmail}
                                        onChange={e => setInviteEmail(e.target.value)}
                                        placeholder="expert@university.edu"
                                        required
                                        autoFocus
                                        className="w-full px-4 py-2.5 rounded-lg border border-[#d4c8bc] bg-white text-[#2c2217] placeholder-[#b0a090] text-sm focus:outline-none focus:border-[#6b1f2a] focus:ring-1 focus:ring-[#6b1f2a] transition-all"
                                    />
                                </div>
                                <div>
                                    <label className="block text-xs font-semibold text-[#5a4535] uppercase tracking-wider mb-1.5">
                                        Expert Name <span className="text-[#b0a090] normal-case font-normal">(optional)</span>
                                    </label>
                                    <input
                                        type="text"
                                        value={inviteName}
                                        onChange={e => setInviteName(e.target.value)}
                                        placeholder="Dr. Jane Smith"
                                        className="w-full px-4 py-2.5 rounded-lg border border-[#d4c8bc] bg-white text-[#2c2217] placeholder-[#b0a090] text-sm focus:outline-none focus:border-[#6b1f2a] focus:ring-1 focus:ring-[#6b1f2a] transition-all"
                                    />
                                </div>
                                <div>
                                    <label className="block text-xs font-semibold text-[#5a4535] uppercase tracking-wider mb-1.5">
                                        Bio <span className="text-[#b0a090] normal-case font-normal">(optional)</span>
                                    </label>
                                    <textarea
                                        value={inviteBio}
                                        onChange={e => setInviteBio(e.target.value)}
                                        placeholder="Brief description of expertise…"
                                        rows={2}
                                        className="w-full px-4 py-2.5 rounded-lg border border-[#d4c8bc] bg-white text-[#2c2217] placeholder-[#b0a090] text-sm focus:outline-none focus:border-[#6b1f2a] focus:ring-1 focus:ring-[#6b1f2a] transition-all resize-none"
                                    />
                                </div>

                                {inviteError && (
                                    <div className="flex items-center gap-2 bg-red-50 border border-red-200 text-red-700 rounded-xl px-3 py-2 text-xs">
                                        <AlertCircle className="w-4 h-4 shrink-0" />
                                        {inviteError}
                                    </div>
                                )}

                                <div className="flex gap-3 pt-1">
                                    <button
                                        type="button"
                                        onClick={closeInvite}
                                        className="flex-1 px-4 py-2.5 rounded-lg border border-[#d4c8bc] text-[#5a4535] text-sm font-medium hover:bg-[#ede6dc] transition-all duration-150"
                                    >
                                        Cancel
                                    </button>
                                    <button
                                        type="submit"
                                        disabled={inviteSubmitting || !inviteEmail.trim()}
                                        className="flex-1 px-4 py-2.5 rounded-lg bg-[#6b1f2a] text-white text-sm font-semibold hover:bg-[#4e1520] active:scale-95 transition-all duration-150 disabled:opacity-50 inline-flex items-center justify-center gap-2"
                                    >
                                        {inviteSubmitting
                                            ? <><Loader2 className="w-4 h-4 animate-spin" /> Inviting…</>
                                            : <><Send className="w-4 h-4" /> Send Invitation</>}
                                    </button>
                                </div>
                            </form>
                        )}
                    </div>
                </div>
            )}
        </div>
    );
}
